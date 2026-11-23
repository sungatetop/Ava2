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
};

export const BTN_DANGER: CSSProperties = { ...BTN, borderColor: "var(--ava-state-danger)", color: "var(--ava-state-danger)" };

export const BTN_PRIMARY: CSSProperties = {
  ...BTN,
  background: "var(--ava-state-business-primary)",
  color: "#fff",
  borderColor: "transparent",
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
  width: "100%",
  fontFamily: "var(--ava-font-ui)",
};

export const TEXTAREA: CSSProperties = { ...INPUT, height: 64, padding: "6px 8px", resize: "vertical", lineHeight: 1.5 };

export const FIELD: CSSProperties = { display: "flex", flexDirection: "column", gap: 4, flex: 1, minWidth: 160 };

export const LABEL: CSSProperties = { fontSize: 11, color: "var(--ava-label-tertiary)" };

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
