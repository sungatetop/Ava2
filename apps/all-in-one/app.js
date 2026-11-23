// 全组合示例 App：一个 App 同时贡献 UI 页（contributes.apps）、技能（skills）、
// hooks（UserPromptSubmit）与 MCP server（toolbox）四类能力。
// UI 部分与 hello 同构：裸 ESM + import map 供给 react（单实例不变式）。
import * as React from "react";

const { createElement: h, useEffect, useState } = React;

const CAPABILITIES = [
  { key: "contributes.apps", by: "前端宿主（页面装配）", what: "导航 + 主区视图（本页面）" },
  { key: "contributes.skills", by: "边车（会话装配期）", what: "技能 toolbox-echo，教 agent 用 echo 工具" },
  { key: "contributes.hooks", by: "边车（会话装配期）", what: "UserPromptSubmit 注入协作提示" },
  { key: "contributes.mcpServers", by: "边车（会话装配期）", what: "stdio server `toolbox`（mcp__toolbox__* 工具）" },
];

export function setup(ctx) {
  function Main() {
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
      { style: { padding: "var(--ava-space-3, 12px)", overflow: "auto", height: "100%" } },
      h("h2", { style: { marginTop: 0 } }, "全组合示例（all-in-one）"),
      h(
        "p",
        { style: { opacity: 0.75, fontSize: 13, lineHeight: "20px" } },
        "广义 App = app（UI/UX）+ skills + hooks + mcpServers 的任意组合。本 App 四类齐备：UI 由前端宿主装配，",
        "其余三类由边车在每次会话装配期注入——安装 / 启停后新会话即时生效。",
      ),
      h(
        "table",
        { style: { borderCollapse: "collapse", fontSize: 12, margin: "var(--ava-space-3, 12px) 0", width: "100%" } },
        h(
          "thead",
          null,
          h(
            "tr",
            null,
            ["贡献声明", "装配方", "内容"].map((label) =>
              h(
                "th",
                { key: label, style: { textAlign: "left", padding: "4px 8px", opacity: 0.6, fontWeight: 500 } },
                label,
              ),
            ),
          ),
        ),
        h(
          "tbody",
          null,
          CAPABILITIES.map((row) =>
            h(
              "tr",
              { key: row.key },
              h("td", { style: { padding: "4px 8px", fontFamily: "var(--ava-font-mono, monospace)" } }, row.key),
              h("td", { style: { padding: "4px 8px", opacity: 0.75 } }, row.by),
              h("td", { style: { padding: "4px 8px", opacity: 0.75 } }, row.what),
            ),
          ),
        ),
      ),
      h("p", { style: { fontSize: 13 } }, "默认模型：", h("b", null, model)),
      h(
        "p",
        { style: { fontSize: 12, opacity: 0.7, lineHeight: "20px" } },
        "端到端验证：新建会话，让 agent 调用 mcp__toolbox__echo（技能 toolbox-echo 有步骤说明）；",
        "每次发消息都会被本 App 的 UserPromptSubmit 钩子附加协作提示（见会话轨迹的 hook_result）。",
      ),
    );
  }

  const disposers = [
    ctx.slots.inject("nav.sidebar", { order: 20, viewId: "all-in-one", title: "全组合" }),
    ctx.slots.inject("surface.main", {
      viewId: "all-in-one",
      title: "全组合示例",
      component: async () => ({ default: Main }),
    }),
  ];

  return {
    dispose() {
      disposers.forEach((d) => d.dispose());
    },
  };
}
