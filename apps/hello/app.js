// 契约冒烟 App（参考实现）：不依赖构建工具，裸 ESM + import map 供给 react。
// 用于验证「宿主动态 import → setup(ctx) → slot 渲染 → hooks 可用（单实例 React）→ AgentChannel 可达」。
import * as React from "react";

const { createElement: h, useEffect, useState } = React;

export function setup(ctx) {
  function Main() {
    const [count, setCount] = useState(0);
    const [model, setModel] = useState("查询中…");

    useEffect(() => {
      let alive = true;
      ctx.agent
        .listModels()
        .then((r) => alive && setModel(r.defaultModel || "（未配置）"))
        .catch((e) => alive && setModel(`不可达：${e.message}`));
      return () => {
        alive = false;
      };
    }, []);

    return h(
      "div",
      { style: { padding: "var(--ava-space-2, 8px)" } },
      h("h2", null, "AppHost 契约冒烟"),
      h("p", null, "本视图由 hello App 经 slots.inject('surface.main') 提供；hooks 正常即证明 React 单实例。"),
      h(
        "p",
        null,
        "计数 ",
        h("b", null, String(count)),
        h("button", { style: { marginLeft: "var(--ava-space-2, 8px)" }, onClick: () => setCount((c) => c + 1) }, "+1"),
      ),
      h("p", null, "默认模型：", h("b", null, model)),
    );
  }

  // 画布第二舞台视图（surface.canvas）：会话总览，验证画布 slot 契约 + agent 通道在画布视图可用
  function Canvas() {
    const [sessions, setSessions] = useState(null);
    const [err, setErr] = useState("");

    useEffect(() => {
      let alive = true;
      ctx.agent
        .listSessions()
        .then((r) => alive && setSessions(Array.isArray(r) ? r : r?.sessions ?? []))
        .catch((e) => alive && setErr(e.message || String(e)));
      return () => {
        alive = false;
      };
    }, []);

    return h(
      "div",
      { style: { padding: "var(--ava-space-2, 8px)" } },
      h("h3", { style: { margin: "0 0 var(--ava-space-2, 8px)" } }, "会话总览"),
      h("p", { style: { margin: "0 0 var(--ava-space-2, 8px)", fontSize: 12, opacity: 0.7 } },
        "本视图由 hello App 经 slots.inject('surface.canvas') 提供（画布第二舞台）。",
      ),
      err && h("p", { style: { color: "#ec1313", fontSize: 12 } }, "加载失败：", err),
      !err && sessions === null && h("p", { style: { fontSize: 12, opacity: 0.6 } }, "加载中…"),
      !err && Array.isArray(sessions) && sessions.length === 0 &&
        h("p", { style: { fontSize: 12, opacity: 0.6 } }, "暂无会话"),
      !err && Array.isArray(sessions) && sessions.length > 0 &&
        h(
          "ul",
          { style: { margin: 0, paddingLeft: "var(--ava-space-4, 16px)", fontSize: 12, lineHeight: "22px" } },
          sessions.slice(0, 8).map((s) =>
            h("li", { key: s.id }, s.title || s.id, h("span", { style: { opacity: 0.5 } }, `（${s.id}）`)),
          ),
        ),
    );
  }

  // 画布面板（panel.canvas）：跟随画布视图 viewId 渲染在舞台下方，验证面板 slot 契约
  function CanvasPanel() {
    const [model, setModel] = useState("查询中…");
    useEffect(() => {
      let alive = true;
      ctx.agent
        .listModels()
        .then((r) => alive && setModel(r.defaultModel || "（未配置）"))
        .catch((e) => alive && setModel(`不可达：${e.message}`));
      return () => {
        alive = false;
      };
    }, []);
    return h(
      "div",
      { style: { fontSize: 12, opacity: 0.8 } },
      h("b", null, "运行状态："),
      "默认模型 ",
      model,
    );
  }

  const disposers = [
    ctx.slots.inject("nav.sidebar", { order: 10, viewId: "hello", title: "冒烟" }),
    ctx.slots.inject("surface.main", { viewId: "hello", title: "冒烟", component: async () => ({ default: Main }) }),
    ctx.slots.inject("surface.canvas", {
      viewId: "sessions",
      title: "会话总览",
      description: "冒烟 App 的画布视图：经 agent 通道拉取会话列表",
      component: async () => ({ default: Canvas }),
    }),
    ctx.slots.inject("panel.canvas", {
      order: 10,
      viewId: "sessions",
      title: "运行状态",
      component: async () => ({ default: CanvasPanel }),
    }),
  ];
  // 事件总线自检：监听宿主广播（观察总线可用 + 卸载时自动排空）
  disposers.push(ctx.events.on("ping", (e) => console.info("[hello] ping", e)));

  return {
    dispose() {
      disposers.forEach((d) => d.dispose());
    },
  };
}
