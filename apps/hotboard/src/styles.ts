/**
 * 热榜聚合 · 样式（形态 B 构建型，React）
 * 设计基线：app-devkit references/standards.md §2.4 形态 B 骨架 + references/DESIGN.md
 * - 全部经 --ava-* 令牌取色取距，明暗自适应，零硬编码色；
 * - 语义标签胶囊用 color-mix 调浅状态色（壳 src/App.css 既有先例）；
 * - 0.5px 发丝线、120–200ms ease 过渡、权重仅 400/500/600/700；
 * - 无 spinner：加载态为脉冲骨架；全部选择器收在 .hb-root 之下。
 */
export const css = `
.hb-root { box-sizing: border-box; height: 100%; display: flex; flex-direction: column; padding: var(--ava-space-4); gap: var(--ava-space-3); overflow: hidden; }
.hb-root *, .hb-root *::before, .hb-root *::after { box-sizing: inherit; }

/* ── 工具栏：分段控件在左，12px 状态文案 + 胶囊按钮在右 ── */
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

/* ── 收藏条：胶囊 chip 可点击打开、× 移除 ── */
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

/* ── 榜单主体：TOP3 卡片区固定在上，其余条目独立滚动 ── */
.hb-list-wrap { flex: 1; min-height: 0; display: flex; flex-direction: column; overflow: hidden; border: 0.5px solid var(--ava-border-l2); border-radius: var(--ava-radius-md); background: var(--ava-bg-layer-1); }
.hb-body { flex: 1; min-height: 0; display: flex; flex-direction: column; gap: var(--ava-space-2); padding: var(--ava-space-2); }
.hb-scroll { flex: 1; min-height: 0; overflow-y: auto; scrollbar-width: thin; scrollbar-color: var(--ava-scrollbar-thumb) transparent; }
.hb-scroll::-webkit-scrollbar { width: 8px; }
.hb-scroll::-webkit-scrollbar-track { background: transparent; }
.hb-scroll::-webkit-scrollbar-thumb { background: var(--ava-scrollbar-thumb); border-radius: var(--ava-radius-pill); }
.hb-scroll::-webkit-scrollbar-thumb:hover { background: var(--ava-scrollbar-thumb-hover); }
/* 骨架屏与空/错态自身滚动（不经 .hb-scroll） */
.hb-list-wrap > div:not(.hb-body) { overflow-y: auto; }
.hb-top3 { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: var(--ava-space-2); flex: none; }
/* 边框用 inset 阴影绘制而非 border：0.5px 边框在布局中会被取整为 1px，
   导致卡片标题与列表标题左缘差 0.5–1px。阴影不参与布局，对齐恒定。 */
.hb-card { display: flex; align-items: center; gap: var(--ava-space-2); padding: 10px var(--ava-space-3); border-radius: var(--ava-radius-md); background: var(--ava-bg-layer-2); box-shadow: inset 0 0 0 0.5px var(--ava-border-l2); transition: background 150ms ease, box-shadow 150ms ease; }
.hb-card:hover { background: var(--ava-button-floating-hover); box-shadow: inset 0 0 0 0.5px var(--ava-border-l3); }
/* 排名数字块：Top3 用导航激活底 + 主文本色，形成明确的版面条带。
   宽 24 与列表 .hb-rank 一致，保证卡片标题与列表标题左缘对齐。 */
.hb-card-rank { flex: none; width: 24px; height: 24px; border-radius: var(--ava-radius-sm); background: var(--ava-nav-active); color: var(--ava-label-primary); font-size: 13px; line-height: 1; font-weight: 700; font-variant-numeric: tabular-nums; display: grid; place-items: center; }
/* 第 1 名用品牌反色，制造单点强调 */
.hb-card:first-child .hb-card-rank { background: var(--ava-brand); color: var(--ava-bg-layer-1); }
.hb-card-body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 5px; }
/* 卡片标题比列表标题大一档并加粗，建立主次层级 */
.hb-card-title { font-size: 14px; line-height: 1.45; font-weight: 600; color: var(--ava-label-primary); text-decoration: none; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; transition: color 150ms ease; }
.hb-card-title:hover { color: var(--ava-state-business-primary); }
.hb-card-meta { display: flex; align-items: center; gap: 6px; }
/* 卡片内热度条保持定宽：拉伸到与标题同宽会被误读成标题下划线 */
.hb-card-meta .hb-bar { width: 56px; flex: none; }

.hb-list { list-style: none; margin: 0; padding: 0; }
.hb-row { display: flex; align-items: center; gap: var(--ava-space-2); padding: 9px var(--ava-space-3); border-bottom: 0.5px solid var(--ava-border-l1); transition: background 150ms ease; }
.hb-row:last-child { border-bottom: 0; }
.hb-row:hover { background: var(--ava-interactive-bg-hover); }

/* 排名：等宽数字防跳；第 4–10 名略强于尾部，形成第二层阅读梯度 */
.hb-rank { flex: none; width: 24px; text-align: center; font-size: 12px; line-height: 1; font-weight: 500; color: var(--ava-label-tertiary); font-variant-numeric: tabular-nums; }

.hb-item-title { flex: 1; min-width: 0; border: 0; background: transparent; padding: 0; text-align: left; font-size: 13px; line-height: 1.5; color: var(--ava-label-primary); cursor: pointer; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: inherit; text-decoration: none; transition: color 150ms ease; }
.hb-item-title:hover { color: var(--ava-state-business-primary); }

/* 语义标签胶囊：新→success、沸→warning，其余中性；状态色 12% 混层底 */
.hb-tag { flex: none; padding: 1px 6px; border-radius: var(--ava-radius-pill); font-size: 11px; line-height: 1.6; font-weight: 500; background: var(--ava-nav-active); color: var(--ava-label-secondary); white-space: nowrap; }
.hb-tag.ok { background: color-mix(in srgb, var(--ava-state-success) 12%, var(--ava-bg-layer-1)); color: var(--ava-state-success); }
.hb-tag.boil { background: color-mix(in srgb, var(--ava-state-warning) 14%, var(--ava-bg-layer-1)); color: var(--ava-state-warning); }

/* 热度：等宽数字 + 定宽迷你条（单列结构，无数值时不渲染数字只留条） */
.hb-hot { flex: none; font-size: 12px; line-height: 1; color: var(--ava-label-secondary); font-variant-numeric: tabular-nums; white-space: nowrap; min-width: 46px; text-align: right; }
.hb-bar { flex: none; width: 56px; height: 3px; border-radius: var(--ava-radius-pill); background: var(--ava-bg-sidebar); overflow: hidden; }
.hb-bar i { display: block; height: 100%; border-radius: var(--ava-radius-pill); background: var(--ava-state-business-primary); opacity: 0.75; transition: width 200ms ease; }

/* 行内操作：外链 + 收藏 */
.hb-act { flex: none; display: inline-flex; align-items: center; gap: 2px; }
.hb-iconbtn { appearance: none; border: 0; background: transparent; width: 24px; height: 24px; border-radius: var(--ava-radius-xs); cursor: pointer; display: inline-flex; align-items: center; justify-content: center; color: var(--ava-label-tertiary); text-decoration: none; transition: background 150ms ease, color 150ms ease; }
.hb-iconbtn:hover { background: var(--ava-interactive-bg-hover); color: var(--ava-label-primary); }
.hb-iconbtn.faved { color: var(--ava-state-business-primary); }
.hb-iconbtn.faved:hover { color: var(--ava-state-business-primary-hover); }

/* ── 加载 / 错误 / 空态（居中：图标槽 + 13px 文案 + 动作） ── */
.hb-state { padding: var(--ava-space-5); text-align: center; font-size: 13px; line-height: 1.5; color: var(--ava-label-secondary); display: flex; flex-direction: column; align-items: center; gap: var(--ava-space-2); }
.hb-state .icon-slot { width: 44px; height: 44px; border-radius: var(--ava-radius-pill); background: var(--ava-bg-sidebar); border: 0.5px solid var(--ava-border-l2); display: grid; place-items: center; color: var(--ava-label-tertiary); margin-bottom: 2px; }
.hb-state.err .icon-slot { color: var(--ava-state-danger); }
.hb-state .msg { color: var(--ava-label-tertiary); font-size: 12px; word-break: break-all; max-width: 420px; }

/* 骨架：脉冲透明度 .45↔.9 / 1.2s（不用 spinner） */
.hb-sk { height: 38px; margin: 8px var(--ava-space-3); border-radius: var(--ava-radius-sm); background: var(--ava-bg-sidebar); animation: hb-pulse 1.2s ease-in-out infinite alternate; }
.hb-sk:nth-child(even) { width: 85%; margin-left: auto; margin-right: auto; }
@keyframes hb-pulse { from { opacity: 0.45; } to { opacity: 0.9; } }
@media (prefers-reduced-motion: reduce) {
  .hb-sk { animation: none; opacity: 0.7; }
  .hb-row, .hb-tab, .hb-btn, .hb-chip, .hb-iconbtn, .hb-bar i, .hb-item-title { transition: none; }
}

/* ── 脚注：11px 次级说明 ── */
.hb-foot { flex: none; font-size: 11px; line-height: 1.4; color: var(--ava-label-tertiary); display: flex; align-items: center; gap: 6px; }
`;
