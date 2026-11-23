// 用户菜单示例 App（user.menu slot 参考实现）：不依赖构建工具，裸 ESM + import map 供给 react。
// 验证点：user.menu 三种动作形态——openView（打开本 App 主视图）、openSettings（壳级设置弹层）、
// event（宿主代为广播，App 自监听驱动主视图状态）。
import * as React from "react";

const { createElement: h, useEffect, useState } = React;

export function setup(ctx) {
  // 主视图：展示 event 动作的触发记录（订阅由组件挂载期自理，卸载随 dispose 排空）
  function Main() {
    const [pings, setPings] = useState(0);

    useEffect(() => {
      const d = ctx.events.on("usermenu-demo.ping", (e) => {
        console.info("[usermenu-demo] ping", e);
        setPings((n) => n + 1);
      });
      return () => d.dispose();
    }, []);

    return h(
      "div",
      { style: { padding: "var(--ava-space-4, 16px)", fontSize: 14, lineHeight: "24px" } },
      h("h2", { style: { margin: "0 0 var(--ava-space-2, 8px)" } }, "用户菜单示例"),
      h("p", null, "本视图经 slots.inject('surface.main') 提供，导航项经 nav.sidebar 注入。"),
      h("p", null, "侧边栏底部 user 栏菜单中，本 App 贡献了三个菜单项（openView / openSettings / event 三种动作）。"),
      h("p", null, "event 动作触发次数：", h("b", null, String(pings))),
      h(
        "p",
        { style: { fontSize: 12, opacity: 0.65 } },
        "点击 user 栏 → 「发送 ping」即经宿主总线广播 usermenu-demo.ping，本视图自监听并计数。",
      ),
    );
  }

  // 设置面板（openSettings 动作目标）：壳只提供弹层与惰性挂载，表单/存储由 App 自理
  function Settings() {
    const [note, setNote] = useState("");
    const [saved, setSaved] = useState("");

    useEffect(() => {
      ctx.storage.get("note").then((v) => setNote(typeof v === "string" ? v : ""));
    }, []);

    const save = () => {
      ctx.storage.set("note", note);
      setSaved("已保存");
      setTimeout(() => setSaved(""), 1200);
    };

    return h(
      "div",
      null,
      h("p", { style: { fontSize: 13 } }, "示例设置：存储经 ctx.storage（按 appId 命名空间落 localStorage）。"),
      h("input", {
        value: note,
        placeholder: "写点什么…",
        style: {
          width: "100%",
          padding: "6px 10px",
          border: "0.5px solid var(--ava-border-l2)",
          borderRadius: 8,
          fontSize: 13,
        },
        onChange: (e) => setNote(e.target.value),
      }),
      h(
        "button",
        { onClick: save, style: { marginTop: 8 } },
        "保存", saved ? `（${saved}）` : "",
      ),
    );
  }

  const disposers = [
    ctx.slots.inject("nav.sidebar", { order: 90, viewId: "usermenu-demo", title: "菜单示例" }),
    ctx.slots.inject("surface.main", {
      viewId: "usermenu-demo",
      title: "菜单示例",
      component: async () => ({ default: Main }),
    }),
    // ── user.menu：三种动作形态各一项 ──
    ctx.slots.inject("user.menu", {
      id: "open",
      order: 10,
      title: "打开菜单示例",
      icon: "M3 7h4v10H3z M7 7h4v4H7z M7 13h4v4H7z M11 9h10v2H11z M11 15h10v2H11z",
      action: { kind: "openView", viewId: "usermenu-demo" },
    }),
    ctx.slots.inject("user.menu", {
      id: "settings",
      order: 20,
      title: "菜单示例设置",
      icon: "M12 8V4 M12 20v-4 M4.9 4.9l2.8 2.8 M16.3 16.3l2.8 2.8 M2 12h4 M18 12h4 M4.9 19.1l2.8-2.8 M16.3 7.7l2.8-2.8",
      action: { kind: "openSettings" },
    }),
    ctx.slots.inject("user.menu", {
      id: "ping",
      order: 30,
      title: "发送 ping",
      icon: "M22 2L11 13 M22 2l-7 20-4-9-9-4z",
      action: { kind: "event", type: "usermenu-demo.ping", payload: { from: "user.menu" } },
    }),
  ];

  ctx.registerSettingsPanel(async () => ({ default: Settings }));

  return {
    dispose() {
      disposers.forEach((d) => d.dispose());
    },
  };
}
