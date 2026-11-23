// 弹窗画廊 App（demo）：演示同一 App 在五类舞台注入 UI，以及舞台内弹窗控件形态。
// 核心演示点：**弹窗的边界 = 舞台的边界**——每个舞台实例各自持有弹窗状态，
// 遮罩与卡片全部 absolute 于舞台根内：不越出舞台、不遮挡宿主 chrome 与其他 App
//（越权渲染红线见 docs/design/app-floating-windows-design.md §2.2；
// 跨舞台弹层应走 ctx.windows 机制，当前契约未实现，故本 App 只演示舞台内形态）。
// 契约消费：五类 slot 全量注入（同 sessionboard 的消费矩阵），inject 仅 slots（纯 UI 演示）。
import * as React from "react";

const { createElement: h, useEffect, useRef, useState } = React;

// ---- 内联样式（token 化：颜色/字号一律 var(--ava-*)，亮暗主题自动跟随宿主） ----

const btn = {
  padding: "7px 12px",
  border: "0.5px solid var(--ava-border-l2)",
  borderRadius: 8,
  background: "var(--ava-bg-base)",
  color: "var(--ava-label-primary)",
  font: "inherit",
  fontSize: 13,
  cursor: "pointer",
  textAlign: "left",
};

const card = {
  background: "var(--ava-bg-sidebar)",
  border: "0.5px solid var(--ava-border-l2)",
  borderRadius: 12,
  boxShadow: "0 12px 32px rgba(0,0,0,0.18)",
};

const mask = {
  position: "absolute",
  inset: 0,
  background: "rgba(0,0,0,0.4)",
  display: "flex",
  zIndex: 10,
};

// ---- Toast：舞台内底部堆叠，2.6s 自动消失 ----

let toastSeq = 0;

function useToasts() {
  const [toasts, setToasts] = useState([]);
  const push = (text) => {
    const id = ++toastSeq;
    setToasts((t) => [...t, { id, text }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), 2600);
  };
  return { toasts, push };
}

// ---- 弹层进场过渡（两段挂载：先挂载再置位，触发 transform/opacity 过渡，无需全局 keyframes） ----

function useEnter(active) {
  const [shown, setShown] = useState(false);
  useEffect(() => {
    if (!active) {
      setShown(false);
      return;
    }
    const raf = requestAnimationFrame(() => setShown(true));
    return () => cancelAnimationFrame(raf);
  }, [active]);
  return shown;
}

// ---- 气泡确认（锚定按钮下方的小卡片） ----

function AnchorConfirm({ stage, onDone }) {
  const [open, setOpen] = useState(false);
  const ref = useRef(null);
  // 点外面收起（气泡不遮罩，属轻量控件）
  useEffect(() => {
    if (!open) return;
    const onDown = (e) => {
      if (ref.current && !ref.current.contains(e.target)) setOpen(false);
    };
    document.addEventListener("pointerdown", onDown, true);
    return () => document.removeEventListener("pointerdown", onDown, true);
  }, [open]);
  return h(
    "span",
    { ref, style: { position: "relative", display: "inline-block" } },
    h("button", { style: btn, onClick: () => setOpen((v) => !v) }, "气泡确认"),
    open &&
      h(
        "div",
        {
          style: {
            ...card,
            position: "absolute",
            top: "calc(100% + 6px)",
            left: 0,
            width: 220,
            padding: 12,
            fontSize: 12,
            zIndex: 12,
          },
        },
        h("div", { style: { fontWeight: 600, marginBottom: 4 } }, "确认操作？"),
        h("div", { style: { color: "var(--ava-label-secondary)", marginBottom: 10 } }, `该气泡只覆盖「${stage}」内自己的锚点区域。`),
        h(
          "div",
          { style: { display: "flex", gap: 8, justifyContent: "flex-end" } },
          h("button", { style: { ...btn, padding: "4px 10px" }, onClick: () => setOpen(false) }, "取消"),
          h(
            "button",
            {
              style: { ...btn, padding: "4px 10px", color: "var(--ava-state-danger)", borderColor: "var(--ava-state-danger)" },
              onClick: () => {
                setOpen(false);
                onDone("已确认");
              },
            },
            "确认",
          ),
        ),
      ),
  );
}

// ---- Tooltip（hover 提示，锚定按钮上方） ----

function AnchorTip() {
  const [open, setOpen] = useState(false);
  return h(
    "span",
    { style: { position: "relative", display: "inline-block" } },
    h(
      "button",
      { style: btn, onMouseEnter: () => setOpen(true), onMouseLeave: () => setOpen(false), onFocus: () => setOpen(true), onBlur: () => setOpen(false) },
      "Tooltip",
    ),
    open &&
      h(
        "div",
        {
          style: {
            position: "absolute",
            bottom: "calc(100% + 6px)",
            left: "50%",
            transform: "translateX(-50%)",
            background: "var(--ava-label-primary)",
            color: "var(--ava-bg-base)",
            fontSize: 11,
            padding: "3px 8px",
            borderRadius: 6,
            whiteSpace: "nowrap",
            zIndex: 12,
          },
        },
        "我是 hover 提示（舞台内）",
      ),
  );
}

// ---- 画廊主体：每个舞台一个实例，弹窗状态互相独立 ----

function Gallery({ stage, compact }) {
  const [modal, setModal] = useState(false);
  const [drawer, setDrawer] = useState(false);
  const [lightbox, setLightbox] = useState(false);
  const { toasts, push } = useToasts();
  const anyOpen = modal || drawer || lightbox;
  const shown = useEnter(anyOpen);
  const ease = "transform .18s ease, opacity .18s ease";

  const item = (label, desc, onClick) =>
    h(
      "button",
      { style: { ...btn, display: "flex", flexDirection: "column", gap: 2, alignItems: "flex-start" }, onClick },
      h("span", { style: { fontWeight: 600 } }, label),
      h("span", { style: { fontSize: 11, color: "var(--ava-label-tertiary)" } }, desc),
    );

  return h(
    "div",
    { style: { position: "relative", minHeight: "100%", padding: compact ? 12 : 20, boxSizing: "border-box" } },
    // 头部：舞台标识 + 原则提示
    h(
      "div",
      { style: { display: "flex", alignItems: "baseline", gap: 10, marginBottom: compact ? 10 : 16 } },
      h("span", { style: { fontSize: compact ? 13 : 16, fontWeight: 700 } }, `弹窗画廊 · ${stage}`),
      h("span", { style: { fontSize: 11, color: "var(--ava-label-tertiary)" } }, "弹窗的边界 = 舞台的边界"),
    ),
    // 控件网格
    h(
      "div",
      {
        style: {
          display: "grid",
          gridTemplateColumns: compact ? "1fr" : "repeat(auto-fill, minmax(150px, 1fr))",
          gap: compact ? 8 : 12,
          maxWidth: 640,
        },
      },
      item("模态对话框", "遮罩 + 居中卡片", () => setModal(true)),
      item("侧滑抽屉", "右侧滑入面板", () => setDrawer(true)),
      h(AnchorConfirm, { stage, onDone: (msg) => push(`${stage}：${msg}`) }),
      item("Toast 通知", "底部堆叠·自动消失", () => push(`${stage} 的 Toast #${toastSeq + 1}`)),
      h(AnchorTip),
      !compact && item("灯箱", "内容放大遮罩", () => setLightbox(true)),
    ),
    // 原则说明
    !compact &&
      h(
        "p",
        { style: { marginTop: 16, fontSize: 12, color: "var(--ava-label-tertiary)", maxWidth: 640, lineHeight: 1.6 } },
        "同一 App 在五类舞台（主舞台 / 画布舞台 / 侧栏导航 / 侧栏面板 / 画布面板）各有一个画廊实例；每个实例的弹窗遮罩只覆盖自己所在的舞台区域。舞台内 UI 形态由 App 自决；跨舞台的浮层需走宿主窗口机制（ctx.windows，契约待实现）。",
      ),

    // —— 以下弹层全部 absolute 于本舞台根内， zIndex 互不越界 ——

    // 模态对话框：遮罩 + 居中卡片
    modal &&
      h(
        "div",
        { style: { ...mask, opacity: shown ? 1 : 0, transition: ease }, onClick: () => setModal(false) },
        h(
          "div",
          {
            style: {
              ...card,
              margin: "auto",
              width: 340,
              maxWidth: "90%",
              padding: 18,
              transform: shown ? "scale(1)" : "scale(0.96)",
              transition: ease,
            },
            onClick: (e) => e.stopPropagation(),
          },
          h("div", { style: { fontWeight: 700, marginBottom: 8 } }, "模态对话框"),
          h("div", { style: { fontSize: 13, color: "var(--ava-label-secondary)", lineHeight: 1.6, marginBottom: 16 } }, `这是「${stage}」内的模态：遮罩只盖住本舞台，宿主导航与其他 App 不受影响。`),
          h(
            "div",
            { style: { display: "flex", gap: 8, justifyContent: "flex-end" } },
            h("button", { style: btn, onClick: () => setModal(false) }, "取消"),
            h(
              "button",
              {
                style: { ...btn, fontWeight: 600 },
                onClick: () => {
                  setModal(false);
                  push(`${stage}：模态已确认`);
                },
              },
              "确定",
            ),
          ),
        ),
      ),

    // 侧滑抽屉：右侧滑入面板
    drawer &&
      h(
        "div",
        { style: { ...mask, alignItems: "stretch", justifyContent: "flex-end", opacity: shown ? 1 : 0, transition: ease }, onClick: () => setDrawer(false) },
        h(
          "div",
          {
            style: {
              ...card,
              width: 280,
              maxWidth: "80%",
              height: "100%",
              borderRadius: 0,
              padding: 18,
              boxSizing: "border-box",
              transform: shown ? "translateX(0)" : "translateX(40px)",
              transition: ease,
            },
            onClick: (e) => e.stopPropagation(),
          },
          h("div", { style: { fontWeight: 700, marginBottom: 8 } }, "侧滑抽屉"),
          h("div", { style: { fontSize: 13, color: "var(--ava-label-secondary)", lineHeight: 1.6 } }, `「${stage}」右侧滑入的面板。抽屉高度 = 舞台高度，而非视口高度。`),
        ),
      ),

    // 灯箱：内容放大遮罩（仅完整形态舞台）
    lightbox &&
      h(
        "div",
        { style: { ...mask, opacity: shown ? 1 : 0, transition: ease }, onClick: () => setLightbox(false) },
        h(
          "div",
          {
            style: {
              margin: "auto",
              width: "min(70%, 420px)",
              aspectRatio: "16 / 9",
              borderRadius: 12,
              background: "linear-gradient(135deg, var(--ava-state-info, #3b82f6), var(--ava-state-success, #22c55e))",
              transform: shown ? "scale(1)" : "scale(0.96)",
              transition: ease,
              display: "flex",
              alignItems: "flex-end",
              padding: 14,
              boxSizing: "border-box",
              color: "#fff",
              fontSize: 12,
            },
          },
          "灯箱：点击任意处关闭（内容放大类弹窗）",
        ),
      ),

    // Toast 堆叠：舞台内底部居中
    h(
      "div",
      {
        style: {
          position: "absolute",
          left: "50%",
          bottom: compact ? 8 : 14,
          transform: "translateX(-50%)",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          gap: 6,
          zIndex: 11,
          pointerEvents: "none",
        },
      },
      toasts.map((t) =>
        h(
          "div",
          {
            key: t.id,
            style: {
              ...card,
              padding: "6px 14px",
              fontSize: 12,
              boxShadow: "0 6px 18px rgba(0,0,0,0.22)",
            },
          },
          t.text,
        ),
      ),
    ),
  );
}

// ---- setup：五类舞台各登记一个画廊实例 ----

export function setup(ctx) {
  const disposers = [
    // lucide layers：多层堆叠（舞台/弹层）语义
    ctx.slots.inject("nav.sidebar", {
      order: 9,
      viewId: "ui-gallery",
      title: "弹窗画廊",
      icon: {
        d: [
          "m12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83Z",
          "m22 17.65-9.17 4.16a2 2 0 0 1-1.66 0L2 17.65",
          "m22 12.65-9.17 4.16a2 2 0 0 1-1.66 0L2 12.65",
        ],
      },
    }),
    ctx.slots.inject("surface.main", {
      viewId: "ui-gallery",
      title: "弹窗画廊",
      component: async () => ({ default: () => h(Gallery, { stage: "主舞台" }) }),
    }),
    ctx.slots.inject("surface.canvas", {
      viewId: "ui-gallery",
      title: "弹窗画廊·画布",
      description: "画布舞台内的弹窗画廊实例",
      icon: {
        d: [
          "m12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83Z",
          "m22 17.65-9.17 4.16a2 2 0 0 1-1.66 0L2 17.65",
          "m22 12.65-9.17 4.16a2 2 0 0 1-1.66 0L2 12.65",
        ],
      },
      component: async () => ({ default: () => h(Gallery, { stage: "画布舞台" }) }),
    }),
    ctx.slots.inject("panel.sidebar", {
      order: 9,
      viewId: "ui-gallery",
      title: "弹窗画廊",
      component: async () => ({ default: () => h(Gallery, { stage: "侧栏面板", compact: true }) }),
    }),
    ctx.slots.inject("panel.canvas", {
      order: 9,
      viewId: "ui-gallery",
      title: "弹窗画廊",
      component: async () => ({ default: () => h(Gallery, { stage: "画布面板", compact: true }) }),
    }),
  ];
  return {
    dispose() {
      for (const d of disposers) d.dispose();
    },
  };
}
