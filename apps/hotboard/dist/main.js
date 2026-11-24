// src/main.tsx
import { useEffect, useMemo, useRef, useState } from "react";

// src/styles.ts
var css = `
.hb-root { box-sizing: border-box; height: 100%; display: flex; flex-direction: column; padding: var(--ava-space-4); gap: var(--ava-space-3); overflow: hidden; }
.hb-root *, .hb-root *::before, .hb-root *::after { box-sizing: inherit; }

/* \u2500\u2500 \u5DE5\u5177\u680F\uFF1A\u5206\u6BB5\u63A7\u4EF6\u5728\u5DE6\uFF0C12px \u72B6\u6001\u6587\u6848 + \u80F6\u56CA\u6309\u94AE\u5728\u53F3 \u2500\u2500 */
.hb-toolbar { display: flex; align-items: center; gap: var(--ava-space-2); flex: none; }
.hb-tabs { display: inline-flex; align-items: center; gap: 2px; height: 32px; padding: 2px; background: var(--ava-bg-sidebar); border-radius: var(--ava-radius-sm); border: 0.5px solid var(--ava-border-l2); }
.hb-tab { appearance: none; border: 0; background: transparent; height: 26px; padding: 0 12px; border-radius: var(--ava-radius-xs); font-size: 12px; line-height: 1; font-weight: 400; color: var(--ava-label-secondary); cursor: pointer; font-family: inherit; display: inline-flex; align-items: center; white-space: nowrap; transition: background 150ms ease, color 150ms ease; }
.hb-tab:hover { background: var(--ava-interactive-bg-hover); color: var(--ava-label-primary); }
.hb-tab.on { background: var(--ava-nav-active); color: var(--ava-label-primary); font-weight: 500; }
.hb-tab:focus-visible, .hb-btn:focus-visible, .hb-chip:focus-visible, .hb-iconbtn:focus-visible { outline: 2px solid var(--ava-state-business-primary); outline-offset: 1px; }
.hb-spacer { flex: 1; }
.hb-time { font-size: 12px; line-height: 1; color: var(--ava-label-tertiary); font-variant-numeric: tabular-nums; white-space: nowrap; }
.hb-btn { appearance: none; border: 0.5px solid var(--ava-border-l3); background: var(--ava-bg-layer-1); height: 28px; padding: 0 12px; border-radius: var(--ava-radius-pill); font-size: 12px; line-height: 1; color: var(--ava-label-primary); cursor: pointer; font-family: inherit; display: inline-flex; align-items: center; gap: 6px; white-space: nowrap; transition: background 150ms ease, border-color 150ms ease; }
.hb-btn:hover { background: var(--ava-button-floating-hover); }
.hb-btn:active { background: var(--ava-interactive-bg-hover); }
.hb-btn:disabled { opacity: 0.45; cursor: default; }

/* \u2500\u2500 \u6536\u85CF\u6761\uFF1A\u80F6\u56CA chip \u53EF\u70B9\u51FB\u6253\u5F00\u3001\xD7 \u79FB\u9664 \u2500\u2500 */
.hb-favbar { display: flex; align-items: center; gap: var(--ava-space-2); flex: none; }
.hb-favlabel { font-size: 12px; line-height: 1; font-weight: 500; color: var(--ava-label-secondary); flex: none; }
.hb-favchips { display: flex; gap: 6px; overflow-x: auto; flex: 1; min-width: 0; scrollbar-width: thin; scrollbar-color: var(--ava-scrollbar-thumb) transparent; }
.hb-favchips::-webkit-scrollbar { height: 6px; }
.hb-favchips::-webkit-scrollbar-track { background: transparent; }
.hb-favchips::-webkit-scrollbar-thumb { background: var(--ava-scrollbar-thumb); border-radius: var(--ava-radius-pill); }
.hb-chip { appearance: none; border: 0.5px solid var(--ava-border-l2); background: var(--ava-bg-layer-1); border-radius: var(--ava-radius-pill); height: 24px; padding: 0 6px 0 10px; font-size: 12px; line-height: 1; color: var(--ava-label-primary); cursor: pointer; display: inline-flex; align-items: center; gap: 4px; max-width: 220px; text-decoration: none; font-family: inherit; flex: none; transition: background 150ms ease, border-color 150ms ease; }
.hb-chip span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.hb-chip:hover { background: var(--ava-interactive-bg-hover); border-color: var(--ava-border-l3); }
.hb-chipx { appearance: none; border: 0; background: transparent; color: var(--ava-label-tertiary); cursor: pointer; padding: 0; margin-left: 2px; font-size: 13px; line-height: 1; display: inline-flex; align-items: center; justify-content: center; width: 16px; height: 16px; border-radius: var(--ava-radius-pill); transition: color 150ms ease, background 150ms ease; }
.hb-chipx:hover { color: var(--ava-state-danger); background: var(--ava-interactive-bg-hover); }

/* \u2500\u2500 \u699C\u5355\u4E3B\u4F53\uFF1ATOP3 \u5361\u7247\u533A\u56FA\u5B9A\u5728\u4E0A\uFF0C\u5176\u4F59\u6761\u76EE\u72EC\u7ACB\u6EDA\u52A8 \u2500\u2500 */
.hb-list-wrap { flex: 1; min-height: 0; display: flex; flex-direction: column; overflow: hidden; border: 0.5px solid var(--ava-border-l2); border-radius: var(--ava-radius-md); background: var(--ava-bg-layer-1); }
.hb-body { flex: 1; min-height: 0; display: flex; flex-direction: column; gap: var(--ava-space-2); padding: var(--ava-space-2); }
.hb-scroll { flex: 1; min-height: 0; overflow-y: auto; scrollbar-width: thin; scrollbar-color: var(--ava-scrollbar-thumb) transparent; }
.hb-scroll::-webkit-scrollbar { width: 8px; }
.hb-scroll::-webkit-scrollbar-track { background: transparent; }
.hb-scroll::-webkit-scrollbar-thumb { background: var(--ava-scrollbar-thumb); border-radius: var(--ava-radius-pill); }
.hb-scroll::-webkit-scrollbar-thumb:hover { background: var(--ava-scrollbar-thumb-hover); }
/* \u9AA8\u67B6\u5C4F\u4E0E\u7A7A/\u9519\u6001\u81EA\u8EAB\u6EDA\u52A8\uFF08\u4E0D\u7ECF .hb-scroll\uFF09 */
.hb-list-wrap > div:not(.hb-body) { overflow-y: auto; }
.hb-top3 { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: var(--ava-space-2); flex: none; }
/* \u8FB9\u6846\u7528 inset \u9634\u5F71\u7ED8\u5236\u800C\u975E border\uFF1A0.5px \u8FB9\u6846\u5728\u5E03\u5C40\u4E2D\u4F1A\u88AB\u53D6\u6574\u4E3A 1px\uFF0C
   \u5BFC\u81F4\u5361\u7247\u6807\u9898\u4E0E\u5217\u8868\u6807\u9898\u5DE6\u7F18\u5DEE 0.5\u20131px\u3002\u9634\u5F71\u4E0D\u53C2\u4E0E\u5E03\u5C40\uFF0C\u5BF9\u9F50\u6052\u5B9A\u3002 */
.hb-card { display: flex; align-items: center; gap: var(--ava-space-2); padding: 10px var(--ava-space-3); border-radius: var(--ava-radius-md); background: var(--ava-bg-layer-2); box-shadow: inset 0 0 0 0.5px var(--ava-border-l2); transition: background 150ms ease, box-shadow 150ms ease; }
.hb-card:hover { background: var(--ava-button-floating-hover); box-shadow: inset 0 0 0 0.5px var(--ava-border-l3); }
/* \u6392\u540D\u6570\u5B57\u5757\uFF1ATop3 \u7528\u5BFC\u822A\u6FC0\u6D3B\u5E95 + \u4E3B\u6587\u672C\u8272\uFF0C\u5F62\u6210\u660E\u786E\u7684\u7248\u9762\u6761\u5E26\u3002
   \u5BBD 24 \u4E0E\u5217\u8868 .hb-rank \u4E00\u81F4\uFF0C\u4FDD\u8BC1\u5361\u7247\u6807\u9898\u4E0E\u5217\u8868\u6807\u9898\u5DE6\u7F18\u5BF9\u9F50\u3002 */
.hb-card-rank { flex: none; width: 24px; height: 24px; border-radius: var(--ava-radius-sm); background: var(--ava-nav-active); color: var(--ava-label-primary); font-size: 13px; line-height: 1; font-weight: 700; font-variant-numeric: tabular-nums; display: grid; place-items: center; }
/* \u7B2C 1 \u540D\u7528\u54C1\u724C\u53CD\u8272\uFF0C\u5236\u9020\u5355\u70B9\u5F3A\u8C03 */
.hb-card:first-child .hb-card-rank { background: var(--ava-brand); color: var(--ava-bg-layer-1); }
.hb-card-body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 5px; }
/* \u5361\u7247\u6807\u9898\u6BD4\u5217\u8868\u6807\u9898\u5927\u4E00\u6863\u5E76\u52A0\u7C97\uFF0C\u5EFA\u7ACB\u4E3B\u6B21\u5C42\u7EA7 */
.hb-card-title { font-size: 14px; line-height: 1.45; font-weight: 600; color: var(--ava-label-primary); text-decoration: none; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; transition: color 150ms ease; }
.hb-card-title:hover { color: var(--ava-state-business-primary); }
.hb-card-meta { display: flex; align-items: center; gap: 6px; }
/* \u5361\u7247\u5185\u70ED\u5EA6\u6761\u4FDD\u6301\u5B9A\u5BBD\uFF1A\u62C9\u4F38\u5230\u4E0E\u6807\u9898\u540C\u5BBD\u4F1A\u88AB\u8BEF\u8BFB\u6210\u6807\u9898\u4E0B\u5212\u7EBF */
.hb-card-meta .hb-bar { width: 56px; flex: none; }

.hb-list { list-style: none; margin: 0; padding: 0; }
.hb-row { display: flex; align-items: center; gap: var(--ava-space-2); padding: 9px var(--ava-space-3); border-bottom: 0.5px solid var(--ava-border-l1); transition: background 150ms ease; }
.hb-row:last-child { border-bottom: 0; }
.hb-row:hover { background: var(--ava-interactive-bg-hover); }

/* \u6392\u540D\uFF1A\u7B49\u5BBD\u6570\u5B57\u9632\u8DF3\uFF1B\u7B2C 4\u201310 \u540D\u7565\u5F3A\u4E8E\u5C3E\u90E8\uFF0C\u5F62\u6210\u7B2C\u4E8C\u5C42\u9605\u8BFB\u68AF\u5EA6 */
.hb-rank { flex: none; width: 24px; text-align: center; font-size: 12px; line-height: 1; font-weight: 500; color: var(--ava-label-tertiary); font-variant-numeric: tabular-nums; }

.hb-item-title { flex: 1; min-width: 0; border: 0; background: transparent; padding: 0; text-align: left; font-size: 13px; line-height: 1.5; color: var(--ava-label-primary); cursor: pointer; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: inherit; text-decoration: none; transition: color 150ms ease; }
.hb-item-title:hover { color: var(--ava-state-business-primary); }

/* \u8BED\u4E49\u6807\u7B7E\u80F6\u56CA\uFF1A\u65B0\u2192success\u3001\u6CB8\u2192warning\uFF0C\u5176\u4F59\u4E2D\u6027\uFF1B\u72B6\u6001\u8272 12% \u6DF7\u5C42\u5E95 */
.hb-tag { flex: none; padding: 1px 6px; border-radius: var(--ava-radius-pill); font-size: 11px; line-height: 1.6; font-weight: 500; background: var(--ava-nav-active); color: var(--ava-label-secondary); white-space: nowrap; }
.hb-tag.ok { background: color-mix(in srgb, var(--ava-state-success) 12%, var(--ava-bg-layer-1)); color: var(--ava-state-success); }
.hb-tag.boil { background: color-mix(in srgb, var(--ava-state-warning) 14%, var(--ava-bg-layer-1)); color: var(--ava-state-warning); }

/* \u70ED\u5EA6\uFF1A\u7B49\u5BBD\u6570\u5B57 + \u5B9A\u5BBD\u8FF7\u4F60\u6761\uFF08\u5355\u5217\u7ED3\u6784\uFF0C\u65E0\u6570\u503C\u65F6\u4E0D\u6E32\u67D3\u6570\u5B57\u53EA\u7559\u6761\uFF09 */
.hb-hot { flex: none; font-size: 12px; line-height: 1; color: var(--ava-label-secondary); font-variant-numeric: tabular-nums; white-space: nowrap; min-width: 46px; text-align: right; }
.hb-bar { flex: none; width: 56px; height: 3px; border-radius: var(--ava-radius-pill); background: var(--ava-bg-sidebar); overflow: hidden; }
.hb-bar i { display: block; height: 100%; border-radius: var(--ava-radius-pill); background: var(--ava-state-business-primary); opacity: 0.75; transition: width 200ms ease; }

/* \u884C\u5185\u64CD\u4F5C\uFF1A\u5916\u94FE + \u6536\u85CF */
.hb-act { flex: none; display: inline-flex; align-items: center; gap: 2px; }
.hb-iconbtn { appearance: none; border: 0; background: transparent; width: 24px; height: 24px; border-radius: var(--ava-radius-xs); cursor: pointer; display: inline-flex; align-items: center; justify-content: center; color: var(--ava-label-tertiary); text-decoration: none; transition: background 150ms ease, color 150ms ease; }
.hb-iconbtn:hover { background: var(--ava-interactive-bg-hover); color: var(--ava-label-primary); }
.hb-iconbtn.faved { color: var(--ava-state-business-primary); }
.hb-iconbtn.faved:hover { color: var(--ava-state-business-primary-hover); }

/* \u2500\u2500 \u52A0\u8F7D / \u9519\u8BEF / \u7A7A\u6001\uFF08\u5C45\u4E2D\uFF1A\u56FE\u6807\u69FD + 13px \u6587\u6848 + \u52A8\u4F5C\uFF09 \u2500\u2500 */
.hb-state { padding: var(--ava-space-5); text-align: center; font-size: 13px; line-height: 1.5; color: var(--ava-label-secondary); display: flex; flex-direction: column; align-items: center; gap: var(--ava-space-2); }
.hb-state .icon-slot { width: 44px; height: 44px; border-radius: var(--ava-radius-pill); background: var(--ava-bg-sidebar); border: 0.5px solid var(--ava-border-l2); display: grid; place-items: center; color: var(--ava-label-tertiary); margin-bottom: 2px; }
.hb-state.err .icon-slot { color: var(--ava-state-danger); }
.hb-state .msg { color: var(--ava-label-tertiary); font-size: 12px; word-break: break-all; max-width: 420px; }

/* \u9AA8\u67B6\uFF1A\u8109\u51B2\u900F\u660E\u5EA6 .45\u2194.9 / 1.2s\uFF08\u4E0D\u7528 spinner\uFF09 */
.hb-sk { height: 38px; margin: 8px var(--ava-space-3); border-radius: var(--ava-radius-sm); background: var(--ava-bg-sidebar); animation: hb-pulse 1.2s ease-in-out infinite alternate; }
.hb-sk:nth-child(even) { width: 85%; margin-left: auto; margin-right: auto; }
@keyframes hb-pulse { from { opacity: 0.45; } to { opacity: 0.9; } }
@media (prefers-reduced-motion: reduce) {
  .hb-sk { animation: none; opacity: 0.7; }
  .hb-row, .hb-tab, .hb-btn, .hb-chip, .hb-iconbtn, .hb-bar i, .hb-item-title { transition: none; }
}

/* \u2500\u2500 \u811A\u6CE8\uFF1A11px \u6B21\u7EA7\u8BF4\u660E \u2500\u2500 */
.hb-foot { flex: none; font-size: 11px; line-height: 1.4; color: var(--ava-label-tertiary); display: flex; align-items: center; gap: 6px; }
`;

// src/main.tsx
import { jsx, jsxs } from "react/jsx-runtime";
var SOURCES = [
  {
    key: "baidu",
    label: "\u767E\u5EA6\u70ED\u641C",
    url: "https://top.baidu.com/api/board?platform=wise&tab=realtime",
    parse(body) {
      const j = JSON.parse(body);
      const cards = Array.isArray(j?.data?.cards) ? j.data.cards : [];
      const groups = [];
      for (const card of cards) {
        if (!card || !Array.isArray(card.content)) continue;
        for (const g of card.content) {
          if (Array.isArray(g?.content)) groups.push(...g.content);
        }
        if (groups.length) break;
      }
      const items = [];
      let rank = 0;
      for (const it of groups) {
        const title = typeof it.word === "string" ? it.word.trim() : "";
        if (!title) continue;
        if (it.isTop) continue;
        rank += 1;
        const labelTag = it.labelTag && typeof it.labelTag.text === "string" ? it.labelTag.text : "";
        const rawTag = typeof it.newHotName === "string" && it.newHotName || labelTag || "";
        items.push({
          rank,
          title,
          url: typeof it.url === "string" ? it.url : "",
          // 该接口已无 hotScore 字段（实测 50 条 0 命中），热度改由排名归一化推导
          hot: Number(it.hotScore) || 0,
          // 「热」在本榜高频出现（约 1/4 条目），无区分度，降噪丢弃
          tag: rawTag === "\u70ED" ? "" : rawTag
        });
      }
      return items;
    }
  },
  {
    key: "toutiao",
    label: "\u5934\u6761\u70ED\u699C",
    url: "https://www.toutiao.com/hot-event/hot-board/?origin=toutiao_pc",
    parse(body) {
      const j = JSON.parse(body);
      const arr = Array.isArray(j?.data) ? j.data : [];
      const items = [];
      let rank = 0;
      for (const it of arr) {
        const title = typeof it.Title === "string" ? it.Title.trim() : "";
        if (!title) continue;
        rank += 1;
        const label = typeof it.Label === "string" ? it.Label : "";
        items.push({
          rank,
          title,
          url: typeof it.Url === "string" ? it.Url : "",
          hot: Number(it.HotValue) || 0,
          tag: label === "new" ? "\u65B0" : label === "boil" ? "\u6CB8" : ""
        });
      }
      return items;
    }
  }
];
function fmtHot(v) {
  if (!Number.isFinite(v) || v <= 0) return "";
  if (v >= 1e8) return `${(v / 1e8).toFixed(1)} \u4EBF`;
  if (v >= 1e4) return `${Math.round(v / 1e4)} \u4E07`;
  return String(Math.round(v));
}
function fmtClock(ts) {
  const d = new Date(ts);
  const p = (n) => String(n).padStart(2, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}
async function fetchBoard(ctx, src) {
  const res = await ctx.net.fetch({ url: src.url });
  if (res.status !== 200) throw new Error(`\u4E0A\u6E38\u8FD4\u56DE HTTP ${res.status}`);
  return src.parse(res.body);
}
var favKey = (it) => it.url || it.title;
var ICON_REFRESH = "M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8 M21 3v5h-5 M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16 M8 16H3v5";
var ICON_BOOKMARK = "m19 21-5.54-5.54a1 1 0 0 0-1.42-.01l-2.56 2.56 M21 10c0 7-9 13-9 13s-9-6-9-13a9 9 0 0 1 18 0z";
var ICON_EXT = "M15 3h6v6 M10 14 21 3 M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6";
var FLAME = "M8.5 14.5A2.5 2.5 0 0 0 11 12c0-1.38-.5-2-1-3-1.072-2.143-.224-4.054 2-6 .5 2.5 2 4.9 4 6.5 2 1.6 3 3.5 3 5.5a7 7 0 1 1-14 0c0-1.153.433-2.294 1-3a2.5 2.5 0 0 0 2.5 2.5z";
var ICON_ALERT = "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20z M12 8v4 M12 16h.01";
var ICON_INBOX = "M22 12h-6l-2 3h-4l-2-3H2 M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z";
function Svg({ d, filled, size = 16 }) {
  return /* @__PURE__ */ jsx(
    "svg",
    {
      width: size,
      height: size,
      viewBox: "0 0 24 24",
      fill: filled ? "currentColor" : "none",
      stroke: filled ? "none" : "currentColor",
      strokeWidth: 1.8,
      strokeLinecap: "round",
      strokeLinejoin: "round",
      "aria-hidden": true,
      children: /* @__PURE__ */ jsx("path", { d })
    }
  );
}
function TopCard({
  item,
  ratio,
  faved,
  onToggleFav
}) {
  const tagCls = item.tag === "\u65B0" ? " ok" : item.tag === "\u6CB8" ? " boil" : "";
  return /* @__PURE__ */ jsxs("li", { className: "hb-card", children: [
    /* @__PURE__ */ jsx("span", { className: "hb-card-rank", children: item.rank }),
    /* @__PURE__ */ jsxs("div", { className: "hb-card-body", children: [
      item.url ? /* @__PURE__ */ jsx("a", { className: "hb-card-title", href: item.url, target: "_blank", rel: "noreferrer", title: item.title, children: item.title }) : /* @__PURE__ */ jsx("span", { className: "hb-card-title", children: item.title }),
      /* @__PURE__ */ jsxs("div", { className: "hb-card-meta", children: [
        item.tag && /* @__PURE__ */ jsx("span", { className: `hb-tag${tagCls}`, children: item.tag }),
        item.hot > 0 && /* @__PURE__ */ jsx("span", { className: "hb-hot", children: fmtHot(item.hot) }),
        /* @__PURE__ */ jsx("span", { className: "hb-bar", "aria-hidden": "true", children: /* @__PURE__ */ jsx("i", { style: { width: `${ratio}%` } }) })
      ] })
    ] }),
    /* @__PURE__ */ jsxs("span", { className: "hb-act", children: [
      item.url && /* @__PURE__ */ jsx("a", { className: "hb-iconbtn", href: item.url, target: "_blank", rel: "noreferrer", title: "\u6253\u5F00\u539F\u94FE\u63A5", children: /* @__PURE__ */ jsx(Svg, { d: ICON_EXT, size: 15 }) }),
      /* @__PURE__ */ jsx(
        "button",
        {
          type: "button",
          className: `hb-iconbtn${faved ? " faved" : ""}`,
          title: faved ? "\u53D6\u6D88\u6536\u85CF" : "\u6536\u85CF\u70ED\u70B9",
          onClick: onToggleFav,
          children: /* @__PURE__ */ jsx(Svg, { d: ICON_BOOKMARK, size: 15, filled: faved })
        }
      )
    ] })
  ] });
}
function HotboardView({ ctx }) {
  const [active, setActive] = useState(SOURCES[0].key);
  const [items, setItems] = useState({});
  const [ts, setTs] = useState({});
  const [err, setErr] = useState({});
  const [loading, setLoading] = useState(true);
  const [favs, setFavs] = useState([]);
  const [tick, setTick] = useState(Date.now());
  const aliveRef = useRef(true);
  const activeRef = useRef(SOURCES[0].key);
  useEffect(() => {
    activeRef.current = active;
  }, [active]);
  const activeSrc = useMemo(() => SOURCES.find((s) => s.key === active) || SOURCES[0], [active]);
  const favedKeys = useMemo(() => new Set(favs.map(favKey)), [favs]);
  const persist = (key, value) => {
    ctx.storage.set(key, value).catch(() => {
    });
  };
  const load = async (src, quiet) => {
    if (!quiet) setLoading(true);
    setErr((m) => ({ ...m, [src.key]: "" }));
    try {
      const list2 = await fetchBoard(ctx, src);
      if (!aliveRef.current) return;
      setItems((m) => ({ ...m, [src.key]: list2 }));
      const now = Date.now();
      setTs((m) => ({ ...m, [src.key]: now }));
      setTick(now);
      persist(`cache:${src.key}`, { v: 1, items: list2.slice(0, 30), ts: now });
    } catch (e) {
      if (!aliveRef.current) return;
      setErr((m) => ({ ...m, [src.key]: e instanceof Error ? e.message : String(e) }));
    } finally {
      if (aliveRef.current) setLoading(false);
    }
  };
  useEffect(() => {
    aliveRef.current = true;
    (async () => {
      const favList = await ctx.storage.get("favorites").catch(() => void 0);
      const caches = await Promise.all(
        SOURCES.map((s) => ctx.storage.get(`cache:${s.key}`).catch(() => void 0))
      );
      if (!aliveRef.current) return;
      SOURCES.forEach((s, i) => {
        const c = caches[i];
        if (c && Array.isArray(c.items) && c.items.length) {
          setItems((m) => ({ ...m, [s.key]: c.items }));
          setTs((m) => ({ ...m, [s.key]: c.ts || 0 }));
        }
      });
      if (Array.isArray(favList)) setFavs(favList);
    })();
    SOURCES.forEach((s) => load(s));
    const timer = setInterval(() => {
      const cur = SOURCES.find((s) => s.key === activeRef.current);
      if (cur) load(cur, true);
    }, 5 * 60 * 1e3);
    return () => {
      aliveRef.current = false;
      clearInterval(timer);
    };
  }, []);
  const toggleFav = (item) => {
    if (!item.title) return;
    setFavs((list2) => {
      const key = favKey(item);
      const next = list2.some((f) => favKey(f) === key) ? list2.filter((f) => favKey(f) !== key) : [{ title: item.title, url: item.url || "", hot: item.hot || 0, ts: Date.now(), source: activeSrc.label }, ...list2];
      persist("favorites", next.slice(0, 100));
      return next;
    });
  };
  const list = items[active] || [];
  const maxHot = list.reduce((m, it) => Math.max(m, it.hot), 0);
  const heatRatio = (it) => {
    if (maxHot > 0) return Math.max(6, Math.round(it.hot / maxHot * 100));
    return Math.max(6, Math.round(Math.exp(-(it.rank - 1) / 7) * 100));
  };
  return /* @__PURE__ */ jsxs("div", { className: "hb-root", children: [
    /* @__PURE__ */ jsx("style", { children: css }),
    /* @__PURE__ */ jsxs("div", { className: "hb-toolbar", children: [
      /* @__PURE__ */ jsx("div", { className: "hb-tabs", role: "tablist", children: SOURCES.map((s) => /* @__PURE__ */ jsx(
        "button",
        {
          type: "button",
          role: "tab",
          "aria-selected": active === s.key,
          className: `hb-tab${active === s.key ? " on" : ""}`,
          onClick: () => setActive(s.key),
          children: s.label
        },
        s.key
      )) }),
      /* @__PURE__ */ jsx("div", { className: "hb-spacer" }),
      ts[active] ? /* @__PURE__ */ jsxs("span", { className: "hb-time", children: [
        "\u66F4\u65B0 ",
        fmtClock(ts[active]),
        " \xB7 \u6BCF 5 \u5206\u949F\u81EA\u52A8\u5237\u65B0"
      ] }) : /* @__PURE__ */ jsx("span", { className: "hb-time", children: "\u7B49\u5F85\u9996\u6B21\u5237\u65B0\u2026" }),
      /* @__PURE__ */ jsxs(
        "button",
        {
          type: "button",
          className: "hb-btn",
          disabled: loading,
          onClick: () => load(activeSrc),
          title: "\u7ACB\u5373\u5237\u65B0",
          children: [
            /* @__PURE__ */ jsx(Svg, { d: ICON_REFRESH, size: 14 }),
            loading ? "\u5237\u65B0\u4E2D\u2026" : "\u5237\u65B0"
          ]
        }
      )
    ] }),
    favs.length > 0 && /* @__PURE__ */ jsxs("div", { className: "hb-favbar", children: [
      /* @__PURE__ */ jsxs("span", { className: "hb-favlabel", children: [
        "\u5DF2\u6536\u85CF ",
        favs.length
      ] }),
      /* @__PURE__ */ jsx("div", { className: "hb-favchips", children: favs.slice(0, 30).map((f, i) => /* @__PURE__ */ jsxs(
        "a",
        {
          className: "hb-chip",
          href: f.url || void 0,
          target: "_blank",
          rel: "noreferrer",
          title: `${f.source} \xB7 \u70B9\u51FB\u6253\u5F00`,
          children: [
            /* @__PURE__ */ jsx("span", { children: f.title }),
            /* @__PURE__ */ jsx(
              "button",
              {
                type: "button",
                className: "hb-chipx",
                "aria-label": "\u79FB\u9664\u6536\u85CF",
                onClick: (e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  toggleFav(f);
                },
                children: "\xD7"
              }
            )
          ]
        },
        favKey(f) + i
      )) })
    ] }),
    /* @__PURE__ */ jsx("div", { className: "hb-list-wrap", children: loading && !list.length ? /* @__PURE__ */ jsx("div", { children: Array.from({ length: 10 }).map((_, i) => /* @__PURE__ */ jsx("div", { className: "hb-sk" }, i)) }) : err[active] && !list.length ? /* @__PURE__ */ jsxs("div", { className: "hb-state err", children: [
      /* @__PURE__ */ jsx("span", { className: "icon-slot", children: /* @__PURE__ */ jsx(Svg, { d: ICON_ALERT, size: 22 }) }),
      activeSrc.label,
      "\u52A0\u8F7D\u5931\u8D25",
      /* @__PURE__ */ jsx("span", { className: "msg", children: err[active] }),
      /* @__PURE__ */ jsx("button", { type: "button", className: "hb-btn", onClick: () => load(activeSrc), children: "\u91CD\u8BD5" })
    ] }) : list.length === 0 ? /* @__PURE__ */ jsxs("div", { className: "hb-state", children: [
      /* @__PURE__ */ jsx("span", { className: "icon-slot", children: /* @__PURE__ */ jsx(Svg, { d: ICON_INBOX, size: 22 }) }),
      "\u672C\u699C\u5355\u6682\u65E0\u6570\u636E",
      /* @__PURE__ */ jsx("span", { className: "msg", children: "\u4E0A\u6E38\u53EF\u80FD\u8C03\u6574\u4E86\u63A5\u53E3\u7ED3\u6784\uFF0C\u6216\u7F51\u7EDC\u53D7\u9650" })
    ] }) : /* @__PURE__ */ jsxs("div", { className: "hb-body", children: [
      /* @__PURE__ */ jsx("ul", { className: "hb-top3", children: list.slice(0, 3).map((it) => /* @__PURE__ */ jsx(
        TopCard,
        {
          item: it,
          ratio: heatRatio(it),
          faved: favedKeys.has(favKey(it)),
          onToggleFav: () => toggleFav(it)
        },
        favKey(it) || it.title
      )) }),
      /* @__PURE__ */ jsx("div", { className: "hb-scroll", children: /* @__PURE__ */ jsx("ul", { className: "hb-list", children: list.slice(3).map((it) => {
        const faved = favedKeys.has(favKey(it));
        const tagCls = it.tag === "\u65B0" ? " ok" : it.tag === "\u6CB8" ? " boil" : "";
        return /* @__PURE__ */ jsxs("li", { className: "hb-row", children: [
          /* @__PURE__ */ jsx("span", { className: "hb-rank", children: it.rank }),
          it.url ? /* @__PURE__ */ jsx("a", { className: "hb-item-title", href: it.url, target: "_blank", rel: "noreferrer", title: it.title, children: it.title }) : /* @__PURE__ */ jsx("span", { className: "hb-item-title", style: { cursor: "default" }, children: it.title }),
          it.tag && /* @__PURE__ */ jsx("span", { className: `hb-tag${tagCls}`, children: it.tag }),
          it.hot > 0 && /* @__PURE__ */ jsx("span", { className: "hb-hot", children: fmtHot(it.hot) }),
          /* @__PURE__ */ jsx("span", { className: "hb-bar", "aria-hidden": "true", children: /* @__PURE__ */ jsx("i", { style: { width: `${heatRatio(it)}%` } }) }),
          /* @__PURE__ */ jsxs("span", { className: "hb-act", children: [
            it.url && /* @__PURE__ */ jsx("a", { className: "hb-iconbtn", href: it.url, target: "_blank", rel: "noreferrer", title: "\u6253\u5F00\u539F\u94FE\u63A5", children: /* @__PURE__ */ jsx(Svg, { d: ICON_EXT, size: 15 }) }),
            /* @__PURE__ */ jsx(
              "button",
              {
                type: "button",
                className: `hb-iconbtn${faved ? " faved" : ""}`,
                title: faved ? "\u53D6\u6D88\u6536\u85CF" : "\u6536\u85CF\u70ED\u70B9",
                onClick: () => toggleFav(it),
                children: /* @__PURE__ */ jsx(Svg, { d: ICON_BOOKMARK, size: 15, filled: faved })
              }
            )
          ] })
        ] }, favKey(it) || it.title);
      }) }) })
    ] }) }),
    /* @__PURE__ */ jsxs("div", { className: "hb-foot", children: [
      /* @__PURE__ */ jsx(Svg, { d: FLAME, size: 12 }),
      "\u516C\u5F00\u63A5\u53E3\u7ECF\u7EDF\u4E00\u5916\u8054\u670D\u52A1\u76F4\u8FBE \xB7 \u91C7\u96C6\u4E8E ",
      fmtClock(tick)
    ] })
  ] });
}
function setup(ctx) {
  const disposers = [
    ctx.slots.inject("nav.sidebar", {
      order: 12,
      viewId: "hotboard",
      title: "\u70ED\u699C\u805A\u5408",
      // lucide flame
      icon: FLAME
    }),
    ctx.slots.inject("surface.main", {
      viewId: "hotboard",
      title: "\u70ED\u699C\u805A\u5408",
      component: async () => ({ default: () => /* @__PURE__ */ jsx(HotboardView, { ctx }) })
    })
  ];
  return {
    dispose() {
      disposers.forEach((d) => d.dispose());
    }
  };
}
export {
  setup
};
