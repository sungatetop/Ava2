// Popconfirm（对齐 model-manager/sessionboard 模式：fixed 锚点定位 + 点外关闭）。
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { BTN, BTN_DANGER } from "./styles";

interface PopconfirmProps {
  title: string;
  onCancel: () => void;
  onConfirm: () => void;
}

export function Popconfirm({ title, onCancel, onConfirm }: PopconfirmProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null);

  useEffect(() => {
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onCancel();
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [onCancel]);

  useLayoutEffect(() => {
    const el = ref.current;
    const anchor = el?.parentElement;
    if (!el || !anchor) return;
    const r = anchor.getBoundingClientRect();
    const w = el.offsetWidth;
    const bh = el.offsetHeight;
    const left = Math.max(8, Math.min(r.right - w, window.innerWidth - w - 8));
    let top = r.bottom + 6;
    if (top + bh > window.innerHeight - 8) top = Math.max(8, r.top - bh - 6);
    setPos({ top, left });
  }, []);

  return (
    <div
      ref={ref}
      role="dialog"
      aria-label="确认操作"
      onClick={(e) => e.stopPropagation()}
      style={{
        position: "fixed",
        zIndex: 40,
        minWidth: 224,
        maxWidth: 268,
        padding: 10,
        border: "0.5px solid var(--ava-border-l3)",
        borderRadius: "var(--ava-radius-sm, 10px)",
        background: "var(--ava-bg-layer-2)",
        boxShadow: "var(--ava-shadow-panel)",
        fontSize: 12,
        color: "var(--ava-label-primary)",
        ...(pos ? { top: pos.top, left: pos.left } : { visibility: "hidden" }),
      }}
    >
      <div style={{ lineHeight: 1.6, wordBreak: "break-word" }}>{title}</div>
      <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 8 }}>
        <button style={{ ...BTN, color: "var(--ava-label-secondary)" }} onClick={onCancel}>
          取消
        </button>
        <button
          style={BTN_DANGER}
          onClick={() => {
            onCancel();
            onConfirm();
          }}
        >
          确认
        </button>
      </div>
    </div>
  );
}
