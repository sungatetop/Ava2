// 锚定上浮面板（composer 工作区 / 审批模式 / 模型三枚 chip 共用）。
// 面板经 portal 挂到 body，按锚点矩形做 fixed 定位：默认向上翻起，
// 同侧空间不足时翻向另一侧；高度受视口限制，内部滚动。
// 关闭语义：点击面板与锚点之外 / Esc。定位在首帧即算好（不做 visibility 占位），
// 以便子项挂载时就能拿到焦点。
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { CSSProperties, ReactNode } from "react";
import { createPortal } from "react-dom";

/** 面板与锚点的间隙、面板与视口边缘的最小留白 */
const GAP = 8;
const EDGE = 8;
/** 同侧可用空间小于该值时优先翻向另一侧 */
const MIN_SIDE_SPACE = 180;

/** 按锚点矩形计算面板的 fixed 定位与高度上限 */
function place(anchor: HTMLElement, side: "top" | "bottom", width: number): CSSProperties {
  const r = anchor.getBoundingClientRect();
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  const w = Math.min(width, vw - EDGE * 2);
  const left = Math.min(Math.max(EDGE, r.left), vw - w - EDGE);
  const above = r.top - GAP - EDGE;
  const below = vh - r.bottom - GAP - EDGE;
  const up = side === "top" ? above >= MIN_SIDE_SPACE || above >= below : below < MIN_SIDE_SPACE && above > below;
  return up
    ? { left, width: w, bottom: vh - r.top + GAP, maxHeight: Math.max(MIN_SIDE_SPACE, above) }
    : { left, width: w, top: r.bottom + GAP, maxHeight: Math.max(MIN_SIDE_SPACE, below) };
}

interface PopoverProps {
  /** 定位锚点（触发 chip 的 DOM 元素） */
  anchor: HTMLElement | null;
  open: boolean;
  onClose: () => void;
  /** 优先展开方向：底部工具条向上，顶部控件向下 */
  side?: "top" | "bottom";
  /** 面板宽度（视口过窄时自动收窄） */
  width?: number;
  children: ReactNode;
}

export function Popover({ anchor, open, onClose, side = "top", width = 280, children }: PopoverProps) {
  const panelRef = useRef<HTMLDivElement | null>(null);
  // 重排触发源：锚点滚动 / 视口变化时自增，促使下一次渲染重算位置
  const [, bump] = useState(0);

  // 锚点滚动 / 视口变化时重新贴合
  useLayoutEffect(() => {
    if (!open || anchor === null) return;
    const follow = () => bump((n) => n + 1);
    window.addEventListener("resize", follow);
    window.addEventListener("scroll", follow, true);
    return () => {
      window.removeEventListener("resize", follow);
      window.removeEventListener("scroll", follow, true);
    };
  }, [open, anchor, side, width]);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const target = e.target as Node;
      if (panelRef.current?.contains(target)) return;
      if (anchor?.contains(target)) return; // 锚点自己负责开合
      onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open, anchor, onClose]);

  if (!open || anchor === null) return null;

  return createPortal(
    <div ref={panelRef} className="cc-pop cc-menu" style={place(anchor, side, width)}>
      {children}
    </div>,
    document.body,
  );
}
