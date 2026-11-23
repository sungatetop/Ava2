// 控件样式常量（token 消费，禁止硬编码色值）。
import type { CSSProperties } from "react";

export const BTN: CSSProperties = {
  height: 26,
  padding: "0 12px",
  border: "0.5px solid var(--ava-border-l3)",
  borderRadius: "var(--ava-radius-sm, 10px)",
  background: "transparent",
  fontSize: 12,
  color: "var(--ava-label-primary)",
  cursor: "pointer",
  whiteSpace: "nowrap",
};

export const INPUT: CSSProperties = {
  height: 28,
  padding: "0 8px",
  border: "0.5px solid var(--ava-border-l3)",
  borderRadius: "var(--ava-radius-sm, 10px)",
  background: "var(--ava-bg-layer-1)",
  color: "var(--ava-label-primary)",
  fontSize: 12,
  outline: "none",
  width: 200,
  fontFamily: "var(--ava-font-ui)",
};

/** 主操作按钮（accent 实底白字，广场卡片安装等强调动作） */
export const BTN_PRIMARY: CSSProperties = {
  height: 26,
  padding: "0 14px",
  border: "none",
  borderRadius: "var(--ava-radius-sm, 10px)",
  background: "var(--ava-state-business-primary)",
  color: "#ffffff",
  fontSize: 12,
  fontWeight: 600,
  cursor: "pointer",
  whiteSpace: "nowrap",
};

/** 卡片网格容器（广场与本地技能共用） */
export const GRID_STYLE: CSSProperties = {
  display: "grid",
  gridTemplateColumns: "repeat(auto-fill, minmax(240px, 1fr))",
  gap: "var(--ava-space-4, 16px)",
};

/** 状态胶囊（accent 为文字强调色；缺省次级文字色） */
export const CHIP = (accent?: string): CSSProperties => ({
  display: "inline-flex",
  alignItems: "center",
  padding: "1px 8px",
  borderRadius: "var(--ava-radius-pill)",
  fontSize: 11,
  whiteSpace: "nowrap",
  background: "var(--ava-nav-active)",
  color: accent || "var(--ava-label-secondary)",
});
