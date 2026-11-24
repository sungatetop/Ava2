// chatapp 样式：对话视觉规格。
// 以 CSS 字符串形式由 ChatView 以 <style> 内联注入——不触碰宿主 DOM，避免与宿主样式耦合。
// 约定：全部选择器限域在 .cc-root 内，类名统一 cc- 前缀；亮/暗通过 data-theme 切换。
export const chatappCss = `
.cc-root,
.cc-pop {
  /* 宿主 design token 契约（src/styles/tokens.css 的 --ava-*）：App 与 AppHost 共用一套语义令牌，
     亮/暗随 documentElement 的 data-theme 继承；宿主缺失时回退到各 var() 内置的亮色值。 */
  --cc-col: clamp(680px, 64%, 920px);
  --cc-t1: var(--ava-label-primary, #0f1115);
  --cc-t2: var(--ava-label-secondary, #61666b);
  --cc-t3: var(--ava-label-tertiary, #81858c);
  --cc-l1: var(--ava-border-l1, rgba(0, 0, 0, 0.04));
  --cc-l2: var(--ava-border-l2, rgba(0, 0, 0, 0.1));
  --cc-l3: var(--ava-border-l3, rgba(0, 0, 0, 0.12));
  --cc-hover: var(--ava-interactive-bg-hover, rgba(38, 49, 72, 0.06));
  --cc-btn-fill: var(--ava-button-elevated-fill, #ffffff);
  --cc-btn-hover: var(--ava-button-floating-hover, #f1f3f5);
  --cc-blue: var(--ava-state-business-primary, #4176e6);
  --cc-danger: var(--ava-state-danger, #ec1313);
  --cc-success: var(--ava-state-success, #12a150);
  --cc-warn: var(--ava-state-warning, #b26a00);
  --cc-shadow: var(--ava-shadow-elevated, 0 2px 10px rgba(0, 0, 0, 0.06));
  --cc-font: var(--ava-font-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC",
    "Microsoft YaHei", system-ui, sans-serif);
  --cc-mono: var(--ava-font-mono, "SF Mono", "JetBrains Mono", "Fira Code", Consolas, monospace);
  /* App 业务专有色彩（不进宿主契约） */
  --cc-bubble: #edf3fe;
  --cc-code-bg: #f9fafb;
  --cc-inline-code: rgba(0, 0, 0, 0.05);
  --cc-input: #ffffff;
  --cc-warn-bg: rgba(240, 180, 41, 0.12);
  --cc-warn-line: rgba(224, 139, 0, 0.5);

  color: var(--cc-t1);
  font-family: var(--cc-font);
  font-size: 14px;
  line-height: 24px;
}

.cc-root {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

[data-theme="dark"] .cc-root,
[data-theme="dark"] .cc-pop {
  /* 仅覆盖 App 专有色彩；语义令牌由宿主 --ava-* 随主题自动切换 */
  --cc-bubble: #2c2c2e;
  --cc-code-bg: #1b1b1c;
  --cc-inline-code: rgba(255, 255, 255, 0.08);
  --cc-input: #2c2c2e;
  --cc-warn-bg: rgba(240, 180, 41, 0.14);
  --cc-warn-line: rgba(240, 180, 41, 0.45);
}

/* ── 会话头：最小高 76px，左右内边距 20/28 ── */
.cc-head {
  display: flex;
  align-items: center;
  gap: var(--ava-space-4, 16px);
  min-height: 52px;
  padding: var(--ava-space-3, 10px) var(--ava-space-5, 24px) var(--ava-space-3, 10px) var(--ava-space-5, 24px);
  border-bottom: 0.5px solid var(--cc-l3);
  flex: none;
}

/* 中栏视图分段 tab（对话 / 轨迹）：分段控件式，激活底 + 主文本色 */
.cc-viewtabs {
  display: inline-flex;
  align-items: center;
  flex: none;
  gap: 2px;
  height: 30px;
  padding: 2px;
  border: 0.5px solid var(--cc-l2);
  border-radius: var(--ava-radius-sm, 10px);
  background: color-mix(in srgb, var(--cc-t1) 4%, var(--ava-bg-layer-1, #ffffff));
}

.cc-viewtab {
  display: inline-flex;
  align-items: center;
  height: 24px;
  padding: 0 var(--ava-space-3, 12px);
  border: none;
  border-radius: calc(var(--ava-radius-sm, 10px) - 2px);
  background: transparent;
  color: var(--cc-t2);
  font: inherit;
  font-size: 12px;
  line-height: 1;
  white-space: nowrap;
  cursor: pointer;
  transition: background 150ms ease, color 150ms ease;
}

.cc-viewtab:hover {
  background: var(--cc-hover);
  color: var(--cc-t1);
}

.cc-viewtab.on {
  background: var(--ava-nav-active, var(--cc-hover));
  color: var(--cc-t1);
  font-weight: 500;
}

.cc-head-dim {
  color: var(--cc-t3);
  font-size: 12px;
  line-height: 18px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

/* 头部右端：生成状态 + 上下文占比 */
.cc-head-right {
  display: flex;
  align-items: center;
  gap: var(--ava-space-3, 10px);
  margin-left: auto;
  min-width: 0;
}

/* 上下文占比：4px 进度条 + 估算数值（≥70% 警告、≥90% 危险） */
.cc-ctx {
  display: flex;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  flex: none;
  color: var(--cc-t3);
  font-size: 12px;
  line-height: 18px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.cc-ctx-bar {
  display: block;
  width: 56px;
  height: 4px;
  flex: none;
  overflow: hidden;
  border-radius: var(--ava-radius-pill, 999px);
  background: var(--cc-l2);
}

.cc-ctx-bar i {
  display: block;
  height: 100%;
  background: var(--cc-blue);
  transition: width 200ms ease;
}

.cc-ctx.warn {
  color: var(--cc-warn);
}

.cc-ctx.warn .cc-ctx-bar i {
  background: var(--cc-warn);
}

.cc-ctx.danger {
  color: var(--cc-danger);
}

.cc-ctx.danger .cc-ctx-bar i {
  background: var(--cc-danger);
}

/* ── 工作目录选择器（模态）：以「浏览 + 选择」代替手输路径 ── */
.cc-picker-mask {
  position: fixed;
  inset: 0;
  z-index: 40;
  background: rgba(0, 0, 0, 0.24);
  animation: cc-fade-in 0.14s ease;
}

@keyframes cc-fade-in {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

@keyframes cc-picker-in {
  from {
    opacity: 0;
    transform: translate(-50%, -50%) scale(0.97);
  }
  to {
    opacity: 1;
    transform: translate(-50%, -50%);
  }
}

.cc-picker {
  position: fixed;
  z-index: 41;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  animation: cc-picker-in 0.14s ease;
  display: flex;
  flex-direction: column;
  width: min(560px, 88vw);
  max-height: min(560px, 76vh);
  padding: var(--ava-space-4, 16px);
  border: 0.5px solid var(--cc-l2);
  border-radius: var(--ava-radius-lg, 16px);
  background: var(--ava-bg-layer-1, #ffffff);
  box-shadow: var(--ava-shadow-panel, 0 8px 24px rgba(0, 0, 0, 0.08));
  color: var(--cc-t1);
  font-family: var(--cc-font);
  font-size: 13px;
}

.cc-picker-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--ava-space-4, 16px);
  padding: 0 var(--ava-space-0, 2px) var(--ava-space-3, 10px);
}

.cc-picker-title {
  font-size: 14px;
  font-weight: 500;
  line-height: 22px;
}

.cc-picker-x {
  padding: 0 var(--ava-space-1, 4px);
  border: none;
  border-radius: var(--ava-radius-sm, 10px);
  background: transparent;
  color: var(--cc-t3);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}

.cc-picker-x:hover {
  background: var(--cc-hover);
  color: var(--cc-t1);
}

.cc-picker-path {
  margin-bottom: var(--ava-space-2, 8px);
  padding: var(--ava-space-2, 8px) var(--ava-space-3, 10px);
  border-radius: var(--ava-radius-sm, 10px);
  background: var(--cc-hover);
  color: var(--cc-t2);
  font-family: var(--cc-mono);
  font-size: 12px;
  line-height: 18px;
  white-space: nowrap;
  overflow-x: auto;
}

.cc-picker-list {
  flex: 1;
  min-height: 140px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: var(--ava-space-0, 2px);
}

.cc-picker-row {
  display: flex;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  width: 100%;
  height: 32px;
  padding: 0 var(--ava-space-2, 8px);
  border: none;
  border-radius: var(--ava-radius-sm, 10px);
  background: transparent;
  color: var(--cc-t1);
  font: inherit;
  font-size: 13px;
  text-align: left;
  cursor: pointer;
}

.cc-picker-row:hover {
  background: var(--cc-hover);
}

.cc-picker-ic {
  flex: none;
  width: 16px;
  color: var(--cc-t3);
  font-size: 12px;
  text-align: center;
}

.cc-picker-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cc-picker-empty {
  padding: var(--ava-space-4, 16px) var(--ava-space-2, 8px);
  color: var(--cc-t3);
  font-size: 12px;
}

/* 最近使用的工作区：目录选择器内的快捷入口 */
.cc-picker-recent {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  padding: var(--ava-space-2, 8px) var(--ava-space-0, 2px) 0;
}

.cc-picker-recent-label {
  font-size: 11px;
  color: var(--cc-t3);
}

.cc-picker-recent-item {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  height: 24px;
  padding: 0 var(--ava-space-3, 10px);
  border: 0.5px solid var(--cc-l2);
  border-radius: var(--ava-radius-pill, 999px);
  background: transparent;
  color: var(--cc-t2);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}

.cc-picker-recent-item:hover {
  background: var(--cc-hover);
  color: var(--cc-t1);
}

.cc-picker-roots {
  display: flex;
  flex-wrap: wrap;
  gap: var(--ava-space-2, 8px);
  padding: var(--ava-space-2, 8px) var(--ava-space-0, 2px) 0;
}

.cc-picker-root {
  height: 24px;
  padding: 0 var(--ava-space-3, 10px);
  border: 0.5px solid var(--cc-l2);
  border-radius: var(--ava-radius-pill, 999px);
  background: transparent;
  color: var(--cc-t2);
  font: inherit;
  font-size: 11px;
  cursor: pointer;
}

.cc-picker-root:hover {
  background: var(--cc-hover);
  color: var(--cc-t1);
}

.cc-picker-err {
  padding: var(--ava-space-2, 8px) var(--ava-space-0, 2px) 0;
  color: var(--cc-danger);
  font-size: 12px;
}

.cc-picker-foot {
  display: flex;
  justify-content: flex-end;
  gap: var(--ava-space-2, 8px);
  padding-top: var(--ava-space-4, 16px);
}

/* 「浏览系统目录…」：左对齐的次级入口（唤起 OS 原生文件夹选择器） */
.cc-picker-native {
  height: 30px;
  padding: 0 var(--ava-space-4, 16px);
  border: 0.5px solid var(--cc-l2);
  border-radius: var(--ava-radius-pill, 999px);
  background: transparent;
  color: var(--cc-t2);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}

.cc-picker-native:hover:not(:disabled) {
  background: var(--cc-hover);
  color: var(--cc-t1);
}

.cc-picker-native:disabled {
  opacity: 0.4;
  cursor: default;
}

/* 占位：把取消/使用此目录推到右侧，浏览系统目录留在左侧 */
.cc-picker-foot-gap {
  flex: 1;
}

.cc-picker-ok {
  height: 30px;
  padding: 0 var(--ava-space-4, 16px);
  border: none;
  border-radius: var(--ava-radius-pill, 999px);
  background: var(--cc-blue);
  color: #fff;
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}

.cc-picker-cancel {
  height: 30px;
  padding: 0 var(--ava-space-4, 16px);
  border: 0.5px solid var(--cc-l2);
  border-radius: var(--ava-radius-pill, 999px);
  background: transparent;
  color: var(--cc-t2);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}

.cc-picker-cancel:hover {
  background: var(--cc-hover);
  color: var(--cc-t1);
}

.cc-picker-ok:disabled {
  opacity: 0.4;
  cursor: default;
}

/* ── 消息流（列宽 clamp(680,64%,920)，两侧 32px） ── */
.cc-main {
  position: relative;
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
}

.cc-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--ava-space-4, 16px) var(--ava-space-5, 24px) var(--ava-space-2, 8px);
}

.cc-flow {
  display: flex;
  flex-direction: column;
  gap: var(--ava-space-4, 16px);
  max-width: var(--cc-col);
  margin: 0 auto;
}

.cc-empty {
  max-width: var(--cc-col);
  margin: var(--ava-space-6, 32px) auto 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  color: var(--cc-t3);
  font-size: 13px;
  text-align: center;
}

/* 空态图标位：44px 圆角底座，对齐宿主空态组件观感 */
.cc-empty-ico {
  display: grid;
  place-items: center;
  width: 44px;
  height: 44px;
  border-radius: var(--ava-radius-md, 8px);
  background: var(--cc-hover, var(--ava-interactive-bg-hover));
  color: var(--cc-t2, var(--ava-label-secondary));
}

.cc-empty-title {
  color: var(--cc-t2, var(--ava-label-secondary));
  font-weight: 500;
}

/* 用户消息：右对齐气泡（radius 20 / padding 10-16 / 行高 22） */
.cc-user {
  align-self: flex-end;
  max-width: min(calc(var(--cc-col) * 0.702), 82%);
  padding: var(--ava-space-3, 10px) var(--ava-space-4, 16px);
  border-radius: var(--ava-radius-xl, 20px);
  background: var(--cc-bubble);
  font-size: 14px;
  line-height: 22px;
  white-space: pre-wrap;
  word-break: break-word;
}

.cc-user.is-optimistic {
  opacity: 0.7;
}

/* 用户气泡内的图片附件：缩略图并排，圆角与气泡一致 */
.cc-user-images {
  display: flex;
  flex-wrap: wrap;
  gap: var(--ava-space-2, 8px);
  margin-bottom: var(--ava-space-2, 8px);
}

.cc-user-images:last-child {
  margin-bottom: 0;
}

.cc-user-image {
  max-width: 220px;
  max-height: 220px;
  border-radius: var(--ava-radius-md, 14px);
  object-fit: cover;
}

/* 助手消息：无气泡无边距边框，仅 markdown 正文 */
.cc-assistant {
  display: flex;
  flex-direction: column;
  gap: var(--ava-space-4, 16px);
  min-width: 0;
}

.cc-caret {
  display: inline-block;
  width: 7px;
  height: 15px;
  margin-top: calc(var(--ava-space-2, 8px) * -1);
  background: var(--cc-t1);
  animation: cc-blink 1s steps(2, start) infinite;
}

@keyframes cc-blink {
  50% {
    opacity: 0;
  }
}

/* ── 轮次过程区：33px 头 + 半像素下边线，默认折叠 ── */
.cc-process-head {
  display: flex;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  width: 100%;
  height: 33px;
  padding: 0;
  border: none;
  border-bottom: 0.5px solid var(--cc-l2);
  background: transparent;
  color: var(--cc-t3);
  font: inherit;
  font-size: 14px;
  line-height: 20px;
  text-align: left;
  cursor: pointer;
}

.cc-process-head:hover {
  color: var(--cc-t2);
}

.cc-chevron {
  display: inline-grid;
  place-items: center;
  width: 14px;
  height: 14px;
  flex: none;
  transition: transform 0.15s ease;
}

.cc-chevron.open {
  transform: rotate(180deg);
}

.cc-process-meta {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}

.cc-process-body {
  display: flex;
  flex-direction: column;
  gap: var(--ava-space-0, 2px);
  padding: var(--ava-space-2, 8px) 0 0;
}

/* ── 思考行：24px 行高，tertiary → hover secondary ── */
.cc-row {
  display: flex;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  width: 100%;
  height: 24px;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--cc-t3);
  font: inherit;
  font-size: 13px;
  line-height: 20px;
  text-align: left;
  cursor: pointer;
}

.cc-row:hover {
  color: var(--cc-t2);
}

.cc-row-static {
  cursor: default;
}

.cc-row-summary {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 流式摘要微光（dsh TextShimmer 对齐）：文字渐变往复扫过 */
.cc-shimmer {
  background: linear-gradient(
    90deg,
    var(--cc-t3) 0%,
    var(--cc-t1) 45%,
    var(--cc-t3) 100%
  );
  background-size: 200% 100%;
  -webkit-background-clip: text;
  background-clip: text;
  -webkit-text-fill-color: transparent;
  animation: cc-shimmer 1.6s linear infinite;
}

@keyframes cc-shimmer {
  from {
    background-position: 200% 0;
  }
  to {
    background-position: -200% 0;
  }
}

.cc-row-summary.err {
  color: var(--cc-danger);
}

/* todo_write 的并行进行项余数：+N 后缀，不参与省略 */
.cc-tool-extra {
  flex: none;
  color: var(--cc-t3);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}

/* 思考展开正文：markdown 次级排版（dsh 对齐——不再用 pre-wrap，由 cc-md 接管） */
.cc-thinking {
  margin: var(--ava-space-0, 2px) 0 var(--ava-space-2, 8px);
  padding-left: var(--ava-space-5, 24px);
  color: var(--cc-t3);
  font-size: 13px;
  line-height: 20px;
  word-break: break-word;
}

/* ── 工具行 ── */
.cc-tool {
  display: flex;
  flex-direction: column;
}

.cc-tool-name {
  color: var(--cc-t2);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
}

/* 折叠头的分隔点：2×2px 圆点（label caption），靠 flex gap 留白 */
.cc-tool-sep {
  width: 2px;
  height: 2px;
  flex: none;
  border-radius: 50%;
  background: var(--cc-t3);
}

.cc-state-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex: none;
  background: var(--cc-t3);
}

.cc-state-dot.ok {
  background: var(--cc-success);
}

.cc-state-dot.err {
  background: var(--cc-danger);
}

.cc-state-dot.running {
  background: transparent;
  border: 1.5px solid var(--cc-l3);
  border-top-color: var(--cc-blue);
  animation: cc-spin 0.75s linear infinite;
}

@keyframes cc-spin {
  to {
    transform: rotate(360deg);
  }
}

/* 工具详情：半像素 l1 + radius 16 + 代码底色 */
.cc-tool-details {
  max-height: 320px;
  margin: var(--ava-space-0, 2px) 0 var(--ava-space-2, 8px) var(--ava-space-5, 24px);
  overflow: auto;
  border: 0.5px solid var(--cc-l1);
  border-radius: var(--ava-radius-lg, 16px);
  background: var(--cc-code-bg);
}

.cc-tool-detail-row {
  padding: var(--ava-space-3, 10px) var(--ava-space-4, 16px);
  border-bottom: 0.5px solid var(--cc-l2);
  font-family: var(--cc-mono);
  font-size: 11px;
  line-height: 19px;
}

.cc-tool-detail-row:last-child {
  border-bottom: none;
}

.cc-tool-detail-label {
  display: block;
  margin-bottom: var(--ava-space-0, 2px);
  color: var(--cc-t3);
  font-family: var(--cc-font);
  font-size: 11px;
}

.cc-tool-detail-row pre {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
}

.cc-tool-detail-row.err pre {
  color: var(--cc-danger);
}

/* ── markdown：h2 19/28、p 16px 0、列表 18px 缩进 ── */
.cc-md {
  min-width: 0;
  font-size: 14px;
  line-height: 24px;
  word-break: break-word;
}

.cc-md > :first-child {
  margin-top: 0;
}

.cc-md > :last-child {
  margin-bottom: 0;
}

.cc-md h1,
.cc-md h2,
.cc-md h3 {
  /* markdown：标题上 32px / 下 16px */
  margin: 32px 0 var(--ava-space-4, 16px);
  font-weight: 700;
  line-height: 1.45;
}

.cc-md h1 {
  font-size: 21px;
}
.cc-md h2 {
  font-size: 19px;
}
.cc-md h3 {
  font-size: 18px;
}
.cc-md h4 {
  margin: var(--ava-space-4, 16px) 0 var(--ava-space-2, 8px);
  font-size: 14px;
  font-weight: 600;
}

.cc-md p {
  margin: var(--ava-space-4, 16px) 0;
}

.cc-md ul,
.cc-md ol {
  margin: var(--ava-space-4, 16px) 0;
  /* markdown：列表缩进 18px，条目间距 6px */
  padding-left: 18px;
}

.cc-md li {
  margin: 6px 0;
}

.cc-md li > p {
  margin: 0;
}

.cc-md hr {
  margin: 32px 0;
  border: none;
  border-top: 0.5px solid var(--cc-l2);
}

.cc-md blockquote {
  margin: var(--ava-space-4, 16px) 0;
  padding-left: 14px;
  border-left: 2px solid var(--cc-t3);
  color: var(--cc-t2);
}

.cc-md a {
  color: var(--cc-blue);
  font-weight: 500;
  text-decoration: none;
}

.cc-md a:hover {
  text-decoration: underline;
}

.cc-md code {
  padding: 0 5px;
  border: 0.5px solid var(--cc-l1);
  border-radius: var(--ava-radius-sm, 10px);
  background: var(--cc-inline-code);
  font-family: var(--cc-mono);
  font-size: 0.875em;
}

.cc-md table {
  width: max-content;
  max-width: 100%;
  margin: var(--ava-space-4, 16px) 0;
  border-collapse: collapse;
  font-size: 13px;
  line-height: 20px;
}

/* markdown 表格：仅下边线、单元格 10/16、首列不留左内边距 */
.cc-md th,
.cc-md td {
  max-width: min(30vw, 320px);
  min-width: 100px;
  padding: 10px var(--ava-space-4, 16px);
  border: none;
  border-bottom: 0.5px solid var(--cc-l2);
  text-align: left;
}

.cc-md th:first-child,
.cc-md td:first-child {
  padding-left: 0;
}

.cc-md th {
  font-weight: 600;
}

/* 代码块：代码底色 + radius 16 + sticky 语言条 */
.cc-code {
  margin: var(--ava-space-4, 16px) 0;
  overflow: hidden;
  border-radius: var(--ava-radius-lg, 16px);
  background: var(--cc-code-bg);
}

.cc-code-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 9px 14px;
  border-bottom: 0.5px solid var(--cc-l1);
  color: var(--cc-t1);
  font-family: var(--cc-mono);
  font-size: 11px;
  line-height: 18px;
}

.cc-code-copy {
  padding: 0;
  border: none;
  background: transparent;
  color: var(--cc-t3);
  font: inherit;
  font-size: 11px;
  cursor: pointer;
}

.cc-code-copy:hover {
  color: var(--cc-t1);
}

.cc-code pre {
  margin: 0;
  padding: var(--ava-space-4, 16px);
  /* 代码块：不横向滚动，长行软换行 */
  overflow-x: hidden;
}

.cc-code pre code {
  padding: 0;
  border: none;
  background: transparent;
  font-family: var(--cc-mono);
  font-size: 11px;
  line-height: 19px;
  white-space: pre-wrap;
  word-break: break-all;
}

/* ── 轮次末尾：复制操作 + 本轮 token 用量 ──
   显隐由 data-actions-reveal 控制（items.tsx 给出）——
   最后一轮「always」常显；历史轮次「hover」仅悬停/聚焦时显示（opacity 保持布局稳定，
   80ms 过渡；无 hover 指针设备一律常显；用量弹层打开时触发器保持可见）。 */
.cc-tail {
  display: flex;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  height: 28px;
}

@media (hover: hover) {
  .cc-tail[data-actions-reveal="hover"] .cc-actions,
  .cc-tail[data-actions-reveal="hover"] .cc-turn-usage {
    opacity: 0;
    transition: opacity 80ms ease;
  }

  .cc-tail[data-actions-reveal="hover"]:hover .cc-actions,
  .cc-tail[data-actions-reveal="hover"]:hover .cc-turn-usage,
  .cc-tail[data-actions-reveal="hover"]:focus-within .cc-actions,
  .cc-tail[data-actions-reveal="hover"]:focus-within .cc-turn-usage,
  .cc-tail[data-actions-reveal="hover"] .cc-turn-usage.open {
    opacity: 1;
  }
}

/* 轮末用量 pill：15px 数据库图标 + 「用量 N tok」，28px 高 /
   6×8 内边距 / 4px 图标间隙，tertiary 色 + tabular 数值，hover 与展开时显交互底色 */
.cc-turn-usage {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
  height: 28px;
  padding: 6px 8px;
  border: none;
  border-radius: var(--ava-radius-sm, 10px);
  background: transparent;
  color: var(--cc-t3);
  font-size: 12px;
  line-height: 24px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
  cursor: pointer;
}

/* 窄列时标签截断为省略号，数据库图标不收缩 */
.cc-turn-usage .cc-turn-usage-label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.cc-turn-usage svg {
  flex: none;
}

.cc-turn-usage:hover,
.cc-turn-usage.open {
  background: var(--cc-hover);
  color: var(--cc-t3);
}

/* 本轮用量明细弹层：标题行左名右精确总量、0.5px 分隔线、
   dl 双列 76px/auto + 1fr，dt tertiary / dd secondary 右对齐 tabular */
.cc-usage-pop {
  padding: var(--ava-space-4, 16px);
  font-size: 12px;
  line-height: 18px;
  color: var(--cc-t2);
  cursor: default;
}

.cc-usage-pop-title {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 8px;
  color: var(--cc-t1);
  font-weight: 500;
}

.cc-usage-pop-name {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.cc-usage-pop-total {
  font-variant-numeric: tabular-nums;
}

.cc-usage-pop-rule {
  margin-bottom: 10px;
  border-top: 0.5px solid var(--cc-l2);
}

.cc-usage-pop-details {
  display: grid;
  grid-template-columns: minmax(76px, auto) minmax(0, 1fr);
  gap: 6px 16px;
  margin: 0;
  color: var(--cc-t3);
}

.cc-usage-pop-details dt,
.cc-usage-pop-details dd {
  min-width: 0;
  margin: 0;
}

.cc-usage-pop-details dd {
  color: var(--cc-t2);
  font-variant-numeric: tabular-nums;
  text-align: right;
}

/* ── 消息操作行：28px / gap 8 / 15px 图标 / hover 显示 ── */
.cc-actions {
  display: flex;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  height: 28px;
  opacity: 0;
  transition: opacity 80ms ease;
}

.cc-assistant:hover .cc-actions,
.cc-actions:focus-within {
  opacity: 1;
}

.cc-icon-btn {
  display: grid;
  place-items: center;
  width: 22px;
  height: 22px;
  padding: 0;
  border: none;
  border-radius: var(--ava-radius-sm, 10px);
  background: transparent;
  color: var(--cc-t3);
  cursor: pointer;
}

.cc-icon-btn:hover {
  background: var(--cc-hover);
  color: var(--cc-t2);
}

/* ── 提示行（模型重试 / 错误） ── */
.cc-note {
  color: var(--cc-warn);
  font-size: 12px;
  line-height: 18px;
}

/* ── 待回填卡片 ── */
.cc-card {
  display: flex;
  flex-direction: column;
  border-radius: var(--ava-radius-xl, 20px);
  background: var(--cc-input);
  box-shadow: var(--cc-shadow);
}

.cc-card.is-approval {
  border: 0.5px solid var(--cc-warn-line);
}

.cc-card-strip {
  padding: var(--ava-space-3, 10px) var(--ava-space-4, 16px);
  border-radius: var(--ava-radius-xl, 20px) var(--ava-radius-xl, 20px) 0 0;
  background: var(--cc-warn-bg);
  color: var(--cc-warn);
  font-size: 13px;
  line-height: 18px;
}

.cc-card-body {
  display: flex;
  flex-direction: column;
  gap: var(--ava-space-2, 8px);
  padding: var(--ava-space-4, 16px) var(--ava-space-4, 16px);
}

.cc-card-title {
  font-size: 15px;
  line-height: 24px;
  font-weight: 500;
}

.cc-card-meta {
  color: var(--cc-t3);
  font-family: var(--cc-mono);
  font-size: 11px;
  line-height: 19px;
  white-space: pre-wrap;
  word-break: break-word;
}

.cc-card-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--ava-space-2, 8px);
  padding: 0 var(--ava-space-4, 16px) var(--ava-space-4, 16px);
}

.cc-btn {
  height: 32px;
  padding: 0 var(--ava-space-4, 16px);
  border: 0.5px solid var(--cc-l2);
  border-radius: var(--ava-radius-pill, 999px);
  background: transparent;
  color: var(--cc-t1);
  font: inherit;
  font-size: 13px;
  cursor: pointer;
}

.cc-btn:hover {
  background: var(--cc-hover);
}

.cc-btn.is-primary {
  border-color: transparent;
  background: var(--cc-blue);
  color: #fff;
}

.cc-btn.is-primary:hover {
  opacity: 0.9;
}

.cc-btn.is-danger:hover {
  border-color: var(--cc-danger);
  color: var(--cc-danger);
}

.cc-card.is-question {
  padding: var(--ava-space-4, 16px) var(--ava-space-5, 24px);
}

.cc-question {
  display: flex;
  flex-direction: column;
  gap: var(--ava-space-2, 8px);
  padding: var(--ava-space-2, 8px) 0;
}

.cc-question-head {
  color: var(--cc-t3);
  font-size: 12px;
  line-height: 18px;
}

.cc-question-text {
  font-size: 14px;
  line-height: 22px;
  font-weight: 500;
}

.cc-option {
  display: flex;
  align-items: center;
  gap: var(--ava-space-3, 10px);
  min-height: 40px;
  padding: 0 var(--ava-space-4, 16px);
  border: 0.5px solid var(--cc-l1);
  border-radius: var(--ava-radius-md, 12px);
  background: transparent;
  color: var(--cc-t1);
  font: inherit;
  font-size: 13px;
  line-height: 20px;
  text-align: left;
  cursor: pointer;
}

.cc-option:hover {
  background: var(--cc-hover);
}

.cc-option.on {
  border-color: var(--cc-l2);
  background: var(--cc-hover);
}

.cc-option-mark {
  display: grid;
  place-items: center;
  width: 20px;
  height: 20px;
  flex: none;
  border: 0.5px solid var(--cc-l2);
  border-radius: var(--ava-radius-sm, 10px);
  color: var(--cc-blue);
  font-family: var(--cc-mono);
  font-size: 11px;
}

.cc-option-desc {
  color: var(--cc-t3);
  font-size: 12px;
  line-height: 18px;
}

.cc-question-custom {
  width: 100%;
  min-height: 34px;
  padding: var(--ava-space-2, 8px) var(--ava-space-3, 10px);
  border: 0.5px solid var(--cc-l2);
  border-radius: var(--ava-radius-sm, 10px);
  background: transparent;
  color: var(--cc-t1);
  font: inherit;
  font-size: 13px;
}

/* ── composer：列宽+32、radius 28、半像素描边 + 软阴影 ── */
.cc-composer-wrap {
  flex: none;
  width: 100%;
  max-width: calc(var(--cc-col) + 32px);
  margin: 0 auto;
  padding: 0 0 var(--ava-space-4, 16px);
}

.cc-composer {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: var(--ava-space-4, 16px);
  padding: var(--ava-space-3, 10px) var(--ava-space-3, 10px) var(--ava-space-2, 8px);
  border: 0.5px solid var(--cc-l2);
  border-radius: var(--ava-radius-xl, 20px);
  background: var(--cc-input);
  box-shadow: var(--cc-shadow);
}

.cc-input {
  width: 100%;
  min-height: 36px;
  max-height: 220px;
  padding: var(--ava-space-1, 4px) var(--ava-space-2, 8px) 0 var(--ava-space-4, 16px);
  border: none;
  outline: none;
  resize: none;
  background: transparent;
  color: var(--cc-t1);
  font: inherit;
  font-size: 14px;
  line-height: 24px;
  caret-color: var(--cc-blue);
}

.cc-input::placeholder {
  color: var(--cc-t3);
}

/* ── 输入卡片底部工具行：左端工作区/审批，右端模型/发送 ── */
.cc-bar {
  display: flex;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  padding: 0 var(--ava-space-0, 2px);
}

.cc-bar-left,
.cc-bar-right {
  display: flex;
  align-items: center;
  gap: var(--ava-space-1, 4px);
  min-width: 0;
}

.cc-bar-right {
  margin-left: auto;
  gap: var(--ava-space-2, 8px);
}

/* chip 触发器：28px 高、radius-sm、
   图标 + 文字 + 右侧展开箭头，点击展开锚定上浮面板。 */
.cc-chip-btn {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  min-width: 0;
  max-width: 220px;
  height: 28px;
  padding: 0 var(--ava-space-5, 24px) 0 var(--ava-space-2, 8px);
  border: none;
  border-radius: var(--ava-radius-sm, 10px);
  background: transparent;
  color: var(--cc-t2);
  font: inherit;
  font-size: 13px;
  line-height: 20px;
  text-align: left;
  cursor: pointer;
}

.cc-chip-btn:hover:not(:disabled),
.cc-chip-btn[aria-expanded="true"] {
  background: var(--cc-hover);
  color: var(--cc-t1);
}

/* 附件按钮（composer 工具条最左）：与 chip 同高 28px 的纯图标按钮 */
.cc-attach-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  /* 全局 button reset 带 padding: 4px 10px，会让 28px 按钮的 content 区只剩 8px，
     图标被 flex 压扁；纯图标按钮必须清零内边距。 */
  padding: 0;
  border: none;
  border-radius: var(--ava-radius-sm, 10px);
  background: transparent;
  color: var(--cc-t2);
  cursor: pointer;
}

.cc-attach-btn:hover {
  background: var(--cc-hover);
  color: var(--cc-t1);
}

/* 待发送附件预览条（composer 内、输入框上方） */
.cc-attach-list {
  display: flex;
  flex-wrap: wrap;
  gap: var(--ava-space-2, 8px);
  padding: 0 var(--ava-space-1, 4px) var(--ava-space-2, 8px);
}

.cc-attach-chip {
  display: inline-flex;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  max-width: 200px;
  height: 32px;
  padding: 0 var(--ava-space-1, 4px) 0 0;
  border-radius: var(--ava-radius-sm, 10px);
  background: var(--cc-hover);
  color: var(--cc-t2);
  font-size: 12px;
  line-height: 18px;
}

.cc-attach-thumb {
  width: 32px;
  height: 32px;
  border-radius: var(--ava-radius-sm, 10px);
  object-fit: cover;
  flex: none;
}

.cc-attach-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cc-attach-x {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  flex: none;
  border: none;
  border-radius: 50%;
  background: transparent;
  color: var(--cc-t3);
  cursor: pointer;
}

.cc-attach-x:hover {
  background: var(--cc-bubble);
  color: var(--cc-t1);
}

.cc-chip-btn:disabled {
  color: var(--cc-t3);
  cursor: default;
}

.cc-chip-icon {
  display: inline-flex;
  flex: none;
  color: var(--cc-t3);
}

.cc-chip-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cc-chip-chevron {
  position: absolute;
  right: var(--ava-space-2, 8px);
  color: var(--cc-t3);
  pointer-events: none;
  transition: transform 120ms ease;
}

.cc-chip-chevron.open {
  transform: rotate(180deg);
}

/* ── 上浮面板（锚定 composer chip；portal 到 body，故样式不依赖 .cc-root 结构） ── */
.cc-menu {
  position: fixed;
  z-index: 1200;
  display: flex;
  flex-direction: column;
  min-width: 200px;
  padding: var(--ava-space-1, 4px);
  border: 0.5px solid var(--cc-l2);
  border-radius: var(--ava-radius-lg, 16px);
  background: var(--ava-bg-layer-1, #ffffff);
  box-shadow: var(--ava-shadow-panel, 0 8px 24px rgba(0, 0, 0, 0.08));
  color: var(--cc-t1);
  overflow: hidden;
}

.cc-menu-path {
  flex: none;
  margin: var(--ava-space-1, 4px) var(--ava-space-1, 4px) var(--ava-space-0, 2px);
  padding: var(--ava-space-2, 8px) var(--ava-space-3, 10px);
  border-radius: var(--ava-radius-sm, 10px);
  background: var(--cc-hover);
  color: var(--cc-t2);
  font-family: var(--cc-mono);
  font-size: 11px;
  line-height: 16px;
  word-break: break-all;
}

.cc-menu-search {
  flex: none;
  margin: var(--ava-space-1, 4px);
  height: 30px;
  padding: 0 var(--ava-space-2, 8px);
  border: 0.5px solid var(--cc-l2);
  border-radius: var(--ava-radius-sm, 10px);
  background: transparent;
  color: var(--cc-t1);
  font: inherit;
  font-size: 12px;
  outline: none;
}

.cc-menu-search::placeholder {
  color: var(--cc-t3);
}

.cc-menu-search:focus {
  border-color: var(--cc-l3);
}

.cc-menu-list {
  display: flex;
  flex-direction: column;
  gap: var(--ava-space-0, 2px);
  min-height: 0;
  overflow-y: auto;
  padding: var(--ava-space-0, 2px) 0;
}

.cc-menu-group {
  display: flex;
  flex-direction: column;
  gap: var(--ava-space-0, 2px);
}

.cc-menu-group-head {
  position: sticky;
  top: 0;
  z-index: 1;
  padding: var(--ava-space-2, 8px) var(--ava-space-2, 8px) var(--ava-space-1, 4px);
  background: var(--ava-bg-layer-1, #ffffff);
  color: var(--cc-t3);
  font-size: 11px;
  line-height: 16px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.cc-menu-item {
  display: flex;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  width: 100%;
  min-height: 34px;
  padding: var(--ava-space-1, 4px) var(--ava-space-2, 8px);
  border: none;
  border-radius: var(--ava-radius-sm, 10px);
  background: transparent;
  color: var(--cc-t1);
  font: inherit;
  font-size: 13px;
  line-height: 18px;
  text-align: left;
  cursor: pointer;
}

.cc-menu-item:hover:not(:disabled),
.cc-menu-item:focus-visible {
  background: var(--cc-hover);
  outline: none;
}

.cc-menu-item:disabled {
  color: var(--cc-t3);
  cursor: default;
}

.cc-menu-item-icon {
  display: inline-flex;
  flex: none;
  color: var(--cc-t3);
}

.cc-menu-item-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cc-menu-check {
  display: inline-grid;
  place-items: center;
  flex: none;
  width: 14px;
  height: 14px;
  color: var(--cc-blue);
}

.cc-menu-sep {
  flex: none;
  height: 0.5px;
  margin: var(--ava-space-1, 4px) var(--ava-space-2, 8px);
  background: var(--cc-l2);
}

.cc-menu-hint {
  flex: none;
  padding: var(--ava-space-1, 4px) var(--ava-space-2, 8px) var(--ava-space-2, 8px);
  color: var(--cc-t3);
  font-size: 12px;
  text-align: center;
}

.cc-menu-empty {
  padding: var(--ava-space-2, 8px);
  color: var(--cc-t3);
  font-size: 12px;
  line-height: 18px;
}

.cc-send {
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  padding: 0;
  border: none;
  border-radius: 50%;
  background: var(--cc-blue);
  color: #fff;
  cursor: pointer;
}

.cc-send:disabled {
  opacity: 0.4;
  cursor: default;
}

/* 生成中：发送键切成停止键（深色实心底 + 方形图标，与可发送态明显区分） */
.cc-send.is-stop {
  background: var(--cc-t1);
  color: var(--ava-bg-layer-1, #ffffff);
}

.cc-error {
  max-width: calc(var(--cc-col) + 32px);
  margin: 0 auto var(--ava-space-2, 8px);
  color: var(--cc-danger);
  font-size: 12px;
  line-height: 18px;
}

.cc-error-box {
  max-width: var(--cc-col);
  margin: 0 auto;
  color: var(--cc-danger);
  font-size: 12px;
  line-height: 18px;
}

/* ── 会话轨迹视图（独立 tab：工具栏 + 泳道概览 + 回合分组记录表） ──
   色标语义（对齐 ava_square）：用户=中灰、消息=蓝、思考=浅蓝、工具=青、
   上下文/系统=中性灰、错误=红。 */
.cc-trj-root {
  /* 本地别名 → 宿主令牌派生（color-mix 随亮/暗主题自动切换） */
  --cc-trj-accent: color-mix(in srgb, var(--cc-blue) 22%, var(--ava-bg-layer-1, #ffffff));

  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background: var(--ava-bg-layer-1, #ffffff);
  color: var(--cc-t1);
  font-size: 12px;
  line-height: 18px;
}

/* 工具栏：搜索 + 记录数 + 折叠全部 */
.cc-trj-toolbar {
  display: flex;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  flex: none;
  height: 40px;
  padding: 0 var(--ava-space-4, 16px);
  border-bottom: 0.5px solid var(--cc-l3);
}

/* 搜索框：图标内嵌（ava_square Search 图标同位） */
.cc-trj-searchbox {
  position: relative;
  flex: none;
  width: 224px;
}

.cc-trj-searchbox svg {
  position: absolute;
  left: 10px;
  top: 50%;
  color: var(--cc-t3);
  transform: translateY(-50%);
  pointer-events: none;
}

.cc-trj-searchbox input {
  width: 100%;
  height: 26px;
  padding: 0 var(--ava-space-2, 8px) 0 28px;
  border: 0.5px solid var(--cc-l2);
  border-radius: var(--ava-radius-xs, 4px);
  background: var(--cc-btn-fill, #ffffff);
  color: var(--cc-t1);
  font: inherit;
  font-size: 12px;
  line-height: 18px;
  outline: none;
}

.cc-trj-searchbox input:focus {
  border-color: color-mix(in srgb, var(--cc-blue) 40%, var(--cc-l2));
}

.cc-trj-count {
  flex: none;
  color: var(--cc-t3);
  font-variant-numeric: tabular-nums;
}

.cc-trj-toggleall {
  flex: none;
  height: 24px;
  margin-left: auto;
  padding: 0 var(--ava-space-2, 8px);
  border: 0.5px solid var(--cc-l2);
  border-radius: var(--ava-radius-xs, 4px);
  background: transparent;
  color: var(--cc-t2);
  font: inherit;
  font-size: 11px;
  line-height: 1;
  cursor: pointer;
}

.cc-trj-toggleall:hover {
  background: var(--cc-hover);
  color: var(--cc-t1);
}

/* 三泳道概览（序列模式）：统计行 + 左标签列 + 右轨道列；
   块位置 = (index-1)/span 等宽排布，不表达时长（§9.3）。 */
.cc-trj-timeline {
  flex: none;
  padding: var(--ava-space-2, 8px) var(--ava-space-3, 12px);
  border-bottom: 0.5px solid var(--cc-l3);
}

.cc-trj-statline {
  margin: 0 0 6px;
  color: var(--cc-t3);
  font-size: 11px;
  line-height: 14px;
  font-variant-numeric: tabular-nums;
}

.cc-trj-lanes {
  display: flex;
  gap: var(--ava-space-2, 8px);
}

.cc-trj-lane-labels {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: 6px;
  width: 32px;
  text-align: right;
  user-select: none;
}

.cc-trj-lane-label {
  color: var(--cc-t3);
  font-size: 10px;
  line-height: 10px;
}

.cc-trj-lane-tracks {
  display: flex;
  flex: 1;
  min-width: 0;
  flex-direction: column;
  gap: 6px;
}

.cc-trj-lane-track {
  position: relative;
  height: 10px;
}

.cc-trj-block {
  position: absolute;
  top: 0;
  bottom: 0;
  min-width: 3px;
  border: none;
  border-radius: 2px;
  padding: 0;
  background: var(--cc-l3);
  cursor: pointer;
  opacity: 0.8;
  transition: opacity 150ms ease;
}

.cc-trj-block:hover {
  opacity: 1;
}

/* 色标语义（对齐 ava_square）：用户=中灰、消息=蓝、思考=浅蓝、
   工具=青（blue×success 混色）、上下文=浅灰、系统=灰、错误=红 */
.cc-trj-block.is-user {
  background: color-mix(in srgb, var(--cc-t1) 45%, var(--ava-bg-layer-1, #ffffff));
}

.cc-trj-block.is-message {
  background: var(--cc-blue);
}

.cc-trj-block.is-think {
  background: color-mix(in srgb, var(--cc-blue) 30%, var(--ava-bg-layer-1, #ffffff));
}

.cc-trj-block.is-tool {
  background: color-mix(in srgb, var(--cc-blue) 45%, var(--cc-success) 55%);
}

.cc-trj-block.is-context {
  background: color-mix(in srgb, var(--cc-t1) 14%, var(--ava-bg-layer-1, #ffffff));
}

.cc-trj-block.is-system {
  background: color-mix(in srgb, var(--cc-t1) 22%, var(--ava-bg-layer-1, #ffffff));
}

.cc-trj-block.is-error {
  background: var(--cc-danger);
}

.cc-trj-block.on {
  opacity: 1;
  box-shadow: 0 0 0 2px var(--cc-blue);
  z-index: 1;
}

/* 回合分组记录表：粘性轮头 + 记录行 */
.cc-trj-table {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.cc-trj-turn {
  display: flex;
  flex-direction: column;
}

/* 粘性轮头：32px（h-8）；进行中左缘 2px accent 条 */
.cc-trj-turnhead {
  position: sticky;
  top: 0;
  z-index: 1;
  display: flex;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  width: 100%;
  height: 32px;
  padding: 0 var(--ava-space-3, 12px);
  border: none;
  border-bottom: 0.5px solid var(--cc-l3);
  background: color-mix(in srgb, var(--ava-bg-layer-1, #ffffff) 95%, transparent);
  color: var(--cc-t1);
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.cc-trj-turnhead:hover {
  background: var(--cc-hover);
}

.cc-trj-turnhead.run {
  box-shadow: inset 2px 0 0 var(--cc-trj-accent);
}

.cc-trj-chev {
  display: inline-flex;
  flex: none;
  color: var(--cc-t3);
}

.cc-trj-turnname {
  flex: none;
  font-size: 13px;
  font-weight: 600;
  line-height: 18px;
}

.cc-trj-turnhead.run .cc-trj-turnname {
  color: color-mix(in srgb, var(--cc-blue) 55%, var(--cc-t3));
}

.cc-trj-reason {
  flex: none;
  padding: 1px 6px;
  border-radius: var(--ava-radius-pill, 999px);
  background: color-mix(in srgb, var(--cc-t1) 6%, var(--ava-bg-layer-1, #ffffff));
  color: var(--cc-t2);
  font-size: 10px;
  line-height: 14px;
  font-weight: 500;
}

.cc-trj-live {
  flex: none;
  color: color-mix(in srgb, var(--cc-blue) 60%, var(--cc-t3));
  font-size: 11px;
}

.cc-trj-turnmeta {
  margin-left: auto;
  display: inline-flex;
  gap: var(--ava-space-2, 8px);
  color: var(--cc-t3);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

/* 记录行：28px，grid [52px 标签 | 1fr 摘要 | auto 指标 | 14px 箭头]（§9.4） */
.cc-trj-rowwrap {
  border-bottom: 0.5px solid var(--cc-l1);
}

.cc-trj-row {
  display: grid;
  grid-template-columns: 52px minmax(0, 1fr) auto 14px;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  width: 100%;
  height: 28px;
  padding: 0 var(--ava-space-3, 12px);
  border: none;
  background: transparent;
  color: var(--cc-t1);
  font: inherit;
  font-size: 13px;
  line-height: 18px;
  text-align: left;
  cursor: pointer;
  transition: background 150ms ease;
}

.cc-trj-row:hover {
  background: color-mix(in srgb, var(--cc-t1) 3%, var(--ava-bg-layer-1, #ffffff));
}

.cc-trj-row.on {
  background: color-mix(in srgb, var(--cc-blue) 8%, var(--ava-bg-layer-1, #ffffff));
}

/* 类别色标 chip：10px 等宽短码（U/MSG/THINK/TOOL/SYS/CTX/ERR），按 kind 上色 */
.cc-trj-tag {
  display: inline-flex;
  align-items: center;
  justify-self: start;
  padding: 1px 4px;
  border-radius: var(--ava-radius-xs, 4px);
  font-family: var(--cc-mono);
  font-size: 10px;
  font-weight: 600;
  line-height: 14px;
  white-space: nowrap;
}

.cc-trj-tag.is-user {
  color: var(--cc-t2);
  background: color-mix(in srgb, var(--cc-t1) 6%, var(--ava-bg-layer-1, #ffffff));
}

.cc-trj-tag.is-message {
  color: var(--cc-blue);
  background: color-mix(in srgb, var(--cc-blue) 10%, var(--ava-bg-layer-1, #ffffff));
}

.cc-trj-tag.is-think {
  color: color-mix(in srgb, var(--cc-blue) 60%, var(--ava-bg-layer-1, #ffffff));
  background: color-mix(in srgb, var(--cc-blue) 8%, var(--ava-bg-layer-1, #ffffff));
}

.cc-trj-tag.is-tool {
  color: color-mix(in srgb, var(--cc-blue) 30%, var(--cc-success) 70%);
  background: color-mix(in srgb, var(--cc-blue) 12%, var(--cc-success) 12%, var(--ava-bg-layer-1, #ffffff));
}

.cc-trj-tag.is-context {
  color: var(--cc-t3);
  background: color-mix(in srgb, var(--cc-t1) 5%, var(--ava-bg-layer-1, #ffffff));
}

.cc-trj-tag.is-system {
  color: var(--cc-t2);
  background: color-mix(in srgb, var(--cc-t1) 6%, var(--ava-bg-layer-1, #ffffff));
}

.cc-trj-tag.is-error {
  color: var(--cc-danger);
  background: color-mix(in srgb, var(--cc-danger) 10%, var(--ava-bg-layer-1, #ffffff));
}

/* 主/次摘要：有次级摘要时标题取 45% 宽并转等宽（ava_square 同款） */
.cc-trj-row-main {
  display: flex;
  align-items: baseline;
  gap: var(--ava-space-2, 8px);
  min-width: 0;
}

.cc-trj-row-title {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cc-trj-row-title.with-sum {
  flex: none;
  max-width: 45%;
  font-family: var(--cc-mono);
  font-size: 12px;
}

.cc-trj-row-sum {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--cc-t3);
  font-size: 11px;
}

.cc-trj-row-meta {
  display: inline-flex;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  color: var(--cc-t3);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.cc-trj-row-meta .is-ok {
  color: var(--cc-success);
  font-weight: 600;
}

.cc-trj-row-meta .is-fail {
  color: var(--cc-danger);
  font-weight: 600;
}

.cc-trj-row-caret {
  display: inline-flex;
  justify-content: center;
  color: var(--cc-t3);
}

/* 展开详情：等宽 pre，最高 256px 内滚（tool 行显示 input/output 原文） */
.cc-trj-detail {
  padding: 0 var(--ava-space-3, 12px) var(--ava-space-2, 8px);
}

.cc-trj-detail pre {
  max-height: 256px;
  margin: 0;
  padding: var(--ava-space-2, 8px);
  overflow: auto;
  border-radius: var(--ava-radius-xs, 4px);
  background: color-mix(in srgb, var(--cc-t1) 3.5%, var(--ava-bg-layer-1, #ffffff));
  color: var(--cc-t2);
  font-family: var(--cc-mono);
  font-size: 11px;
  line-height: 16px;
  white-space: pre-wrap;
  word-break: break-all;
}

.cc-trj-empty {
  padding: var(--ava-space-5, 24px);
  text-align: center;
  color: var(--cc-t3);
  font-size: 12px;
  line-height: 18px;
}

/* ── 侧边栏会话历史（panel.sidebar） ── */
.cc-root.cc-sb {
  gap: var(--ava-space-0, 2px);
  height: 100%;
  min-height: 0;
  overflow: hidden;
}

.cc-sb-new {
  display: flex;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  flex: none;
  height: 38px;
  margin: 0 var(--ava-space-0, 2px) var(--ava-space-2, 8px);
  padding: var(--ava-space-2, 8px) var(--ava-space-4, 16px);
  border: 0.5px solid var(--cc-l2);
  border-radius: var(--ava-radius-md, 12px);
  background: var(--cc-btn-fill);
  color: var(--cc-t1);
  font: inherit;
  font-size: 14px;
  line-height: 22px;
  font-weight: 500;
  cursor: pointer;
}

.cc-sb-new:hover {
  background: var(--cc-btn-hover);
}

.cc-sb-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 列表左侧 4px 内缩（相对侧栏 12px 内边距），右侧留 2px 给滚动条 */
  padding: 0 var(--ava-space-0, 2px) var(--ava-space-4, 16px) var(--ava-space-1, 4px);
}

.cc-sb-group {
  margin-top: var(--ava-space-1, 4px);
}

.cc-sb-group-head {
  display: flex;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  height: 34px;
  padding: 0 var(--ava-space-2, 8px);
  border-radius: var(--ava-radius-sm, 10px);
  color: var(--cc-t2);
  cursor: pointer;
}

.cc-sb-group-head:hover {
  background: var(--cc-hover);
}

.cc-sb-group.is-current .cc-sb-folder {
  color: var(--cc-blue);
}

.cc-sb-chev {
  flex: none;
  color: var(--cc-t3);
  transition: transform 0.15s ease;
}

.cc-sb-chev.open {
  transform: rotate(90deg);
}

.cc-sb-folder {
  display: inline-grid;
  place-items: center;
  width: 16px;
  height: 16px;
  flex: none;
  color: var(--cc-t3);
}

.cc-sb-group-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--cc-t1);
  font-size: 14px;
  line-height: 20px;
}

.cc-sb-add {
  display: none;
  place-items: center;
  width: 16px;
  height: 16px;
  flex: none;
  padding: 0;
  border: none;
  border-radius: var(--ava-radius-xs, 4px);
  background: transparent;
  color: var(--cc-t3);
  cursor: pointer;
}

.cc-sb-group-head:hover .cc-sb-add {
  display: inline-grid;
}

.cc-sb-add:hover {
  background: var(--cc-l1);
  color: var(--cc-t1);
}

.cc-sb-del {
  display: none;
  place-items: center;
  width: 16px;
  height: 16px;
  flex: none;
  padding: 0;
  border: none;
  border-radius: var(--ava-radius-xs, 4px);
  background: transparent;
  color: var(--cc-t3);
  cursor: pointer;
}

.cc-sb-group-head:hover .cc-sb-del {
  display: inline-grid;
}

.cc-sb-del:hover:not(:disabled) {
  background: var(--cc-hover);
  color: var(--cc-danger);
}

.cc-sb-del:disabled {
  opacity: 0.5;
  cursor: default;
}

/* Popconfirm 气泡确认（浮层指南：锚定触发点、点外取消、无遮罩） */
.cc-pc-anchor {
  position: relative;
}

.cc-pc-anchor.head {
  position: absolute;
  right: 0;
}

.cc-pc {
  /* fixed + JS 按锚点 rect 计算（见 session-sidebar.tsx）：避开滚动容器 overflow 裁剪 */
  position: fixed;
  z-index: 40;
  min-width: 224px;
  max-width: 268px;
  padding: var(--ava-space-3, 10px) var(--ava-space-3, 10px);
  border: 0.5px solid var(--cc-l2);
  border-radius: var(--ava-radius-md, 10px);
  background: var(--ava-surface-elevated, #ffffff);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.14);
}

.cc-pc-msg {
  font-size: 12px;
  line-height: 18px;
  color: var(--cc-t1);
  word-break: break-word;
}

.cc-pc-acts {
  display: flex;
  justify-content: flex-end;
  gap: var(--ava-space-2, 8px);
  margin-top: var(--ava-space-2, 8px);
}

.cc-pc-cancel,
.cc-pc-confirm {
  height: 26px;
  padding: 0 var(--ava-space-3, 12px);
  border: 0.5px solid var(--cc-l2);
  border-radius: var(--ava-radius-sm, 8px);
  background: transparent;
  font-size: 12px;
  cursor: pointer;
}

.cc-pc-cancel {
  color: var(--cc-t2);
}

.cc-pc-cancel:hover {
  background: var(--cc-hover);
  color: var(--cc-t1);
}

.cc-pc-confirm {
  border-color: var(--cc-danger);
  color: var(--cc-danger);
}

.cc-pc-confirm:hover {
  background: rgba(236, 19, 19, 0.08);
}

.cc-sb-row {
  display: flex;
  align-items: center;
  width: 100%;
  height: 32px;
  margin-top: var(--ava-space-0, 2px);
  padding: 0 var(--ava-space-2, 8px);
  border: none;
  border-radius: var(--ava-radius-sm, 10px);
  background: transparent;
  color: var(--cc-t2);
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.cc-sb-row:hover,
.cc-sb-row.on {
  background: var(--cc-hover);
  color: var(--cc-t1);
}

.cc-sb-row-title {
  flex: 1;
  min-width: 0;
  margin: 0 var(--ava-space-2, 8px) 0 var(--ava-space-1, 4px);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 14px;
  line-height: 20px;
}

/* 状态点槽位：16px 固定槽，无 live 时留空以维持标题对齐 */
.cc-sb-row-slot {
  display: inline-grid;
  place-items: center;
  width: 16px;
  height: 16px;
  flex: none;
}

/* 运行中会话状态点：10px 圆点 */
.cc-sb-live {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--cc-blue);
}

.cc-sb-row-time {
  flex: none;
  color: var(--cc-t3);
  font-size: 12px;
  line-height: 20px;
  font-variant-numeric: tabular-nums;
}

.cc-sb-more {
  height: 28px;
  margin-top: var(--ava-space-0, 2px);
  padding: 0 var(--ava-space-4, 16px) 0 var(--ava-space-5, 24px);
  border: none;
  border-radius: var(--ava-radius-sm, 10px);
  background: transparent;
  color: var(--cc-t3);
  font: inherit;
  font-size: 12px;
  line-height: 18px;
  text-align: left;
  cursor: pointer;
}

.cc-sb-more:hover {
  color: var(--cc-t2);
}

.cc-sb-empty {
  padding: var(--ava-space-2, 8px);
  color: var(--cc-t3);
  font-size: 12px;
  line-height: 18px;
}

.cc-sb-error {
  margin: var(--ava-space-1, 4px) 0;
  padding: var(--ava-space-1, 4px) var(--ava-space-2, 8px);
  border-radius: var(--ava-radius-sm, 10px);
  background: var(--cc-warn-bg);
  color: var(--cc-danger);
  font-size: 12px;
  line-height: 18px;
  word-break: break-word;
}

/* 会话行容器：hover 时以操作组替换相对时间 */
.cc-sb-item {
  position: relative;
}

.cc-sb-acts {
  position: absolute;
  top: 50%;
  right: var(--ava-space-1, 4px);
  display: none;
  gap: var(--ava-space-0, 2px);
  transform: translateY(-50%);
}

.cc-sb-item:hover .cc-sb-acts {
  display: flex;
}

.cc-sb-item:hover .cc-sb-row-time {
  opacity: 0;
}

.cc-sb-act {
  display: inline-grid;
  place-items: center;
  width: 24px;
  height: 24px;
  padding: 0;
  border: none;
  border-radius: var(--ava-radius-sm, 8px);
  background: transparent;
  color: var(--cc-t3);
  cursor: pointer;
}

.cc-sb-act:hover:not(:disabled) {
  background: var(--cc-l1);
  color: var(--cc-t1);
}

.cc-sb-act.is-danger:hover:not(:disabled) {
  color: var(--cc-danger);
}

.cc-sb-act:disabled {
  opacity: 0.45;
  cursor: default;
}

/* present 交付物卡片（工具行下方） */
.cc-deliverables {
  display: flex;
  flex-direction: column;
  gap: var(--ava-space-1, 4px);
  margin: var(--ava-space-1, 4px) 0 var(--ava-space-1, 4px) var(--ava-space-5, 24px);
}

.cc-deliverable {
  display: flex;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  padding: var(--ava-space-2, 8px) var(--ava-space-3, 12px);
  border: 0.5px solid var(--cc-l2);
  border-radius: var(--ava-radius-md, 10px);
  background: var(--cc-btn-fill);
}

.cc-deliverable-icon {
  display: inline-grid;
  place-items: center;
  flex: none;
  color: var(--cc-t2);
}

.cc-deliverable-body {
  display: flex;
  flex-direction: column;
  flex: 1 1 auto;
  min-width: 0;
}

.cc-deliverable-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 500;
}

.cc-deliverable-desc {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--cc-t3);
  font-size: 12px;
  line-height: 18px;
}

.cc-deliverable-open {
  flex: none;
  height: 26px;
  padding: 0 var(--ava-space-3, 12px);
  border: 0.5px solid var(--cc-l2);
  border-radius: var(--ava-radius-sm, 8px);
  background: transparent;
  color: var(--cc-t1);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}

.cc-deliverable-open:hover {
  background: var(--cc-hover);
}

/* ── 斜杠命令回执（本地执行结果，不进轮次块；error 态转红） ── */
.cc-notice {
  max-width: 100%;
  color: var(--cc-t2);
  font-size: 12px;
  line-height: 20px;
  white-space: pre-wrap;
  word-break: break-word;
}

.cc-notice.is-error {
  color: var(--cc-danger);
}

/* ── 命令条目：右对齐回显气泡 + 结果行 ── */
.cc-cmd {
  display: flex;
  flex-direction: column;
  gap: var(--ava-space-1, 4px);
}

/* 回显气泡里的命令 chip（mono + 浅底） */
.cc-cmd-chip {
  display: inline-block;
  margin-right: 4px;
  padding: 0 5px;
  border-radius: var(--ava-radius-sm, 6px);
  background: color-mix(in srgb, var(--cc-t1) 8%, transparent);
  font-family: var(--cc-mono);
  font-size: 12px;
  line-height: 18px;
}

.cc-cmd-glyph {
  display: inline-flex;
  flex: none;
  color: var(--cc-t3);
}

.cc-cmd-result[data-state="error"] .cc-cmd-glyph {
  color: var(--cc-danger);
}

/* ── 上下文压缩标记：图标 + 标题 + 计数摘要，可展开 ── */
.cc-compaction {
  display: flex;
  flex-direction: column;
  gap: var(--ava-space-1, 4px);
}

.cc-compaction-btn {
  display: flex;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  width: 100%;
  padding: 2px 0;
  border: 0;
  background: transparent;
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.cc-compaction-btn:disabled {
  cursor: default;
}

.cc-compaction-leading {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  flex: none;
  color: var(--cc-t3);
}

.cc-compaction-icon {
  display: inline-flex;
}

.cc-compaction-title {
  flex: none;
  color: var(--cc-t1);
  font-size: 13px;
  line-height: 22px;
}

.cc-compaction-summary {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  color: var(--cc-t3);
  font-size: 12px;
  line-height: 22px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cc-compaction-summary.err {
  color: var(--cc-danger);
}

.cc-compaction-body {
  padding: var(--ava-space-2, 8px) 0 var(--ava-space-2, 8px) 22px;
  border-left: 2px solid var(--cc-l2);
  color: var(--cc-t2);
  font-size: 13px;
  line-height: 20px;
}

/* ── 待办面板（composer 上方 dock 卡片，空清单不占位、默认折叠） ── */
.cc-todopanel {
  margin-bottom: var(--ava-space-2, 8px);
  border: 0.5px solid var(--cc-l2);
  border-radius: var(--ava-radius-md, 12px);
  background: var(--cc-btn-fill);
  box-shadow: var(--cc-shadow);
}

.cc-todopanel-body {
  display: flex;
  flex-direction: column;
  gap: var(--ava-space-2, 8px);
  padding: 6px 12px;
}

.cc-todopanel-head {
  display: flex;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  width: 100%;
  padding: 0;
  border: 0;
  background: transparent;
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.cc-todopanel-glyph {
  display: inline-flex;
  flex: none;
  color: var(--cc-t3);
}

/* 与 GoalBar 标题同规格（13/24 medium primary），三张 dock 卡片读作同一族 */
.cc-todopanel-title {
  flex: none;
  color: var(--cc-t1);
  font-size: 13px;
  line-height: 24px;
  font-weight: 500;
}

.cc-todopanel-progress {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  color: var(--cc-t3);
  font-size: 13px;
  line-height: 20px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cc-todopanel-list {
  display: flex;
  flex-direction: column;
  gap: var(--ava-space-2, 8px);
  max-height: 180px;
  margin: 0;
  padding: 0;
  overflow-y: auto;
  list-style: none;
}

.cc-todopanel-item {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  color: var(--cc-t2);
  font-size: 13px;
  line-height: 20px;
}

.cc-todopanel-dot {
  display: inline-grid;
  place-items: center;
  flex: none;
  width: 16px;
  height: 16px;
}

.cc-todopanel-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ── 待办状态点：完成 = 6px 实心点 / 进行中 = 14px 旋转环 / 待处理 = 中性点 ── */
.cc-statedot {
  display: inline-block;
  flex: none;
  width: 6px;
  height: 6px;
  border-radius: 999px;
}

.cc-statedot.is-done {
  background: var(--cc-success);
}

.cc-statedot.is-idle {
  background: var(--cc-t3);
  opacity: 0.45;
}

.cc-statedot.is-ongoing {
  width: 14px;
  height: 14px;
}

.cc-statedot.is-ongoing svg {
  display: block;
  width: 14px;
  height: 14px;
  animation: cc-statedot-spin 0.9s linear infinite;
}

.cc-statedot-track {
  fill: none;
  stroke: var(--cc-blue);
  stroke-width: 2;
  opacity: 0.25;
}

/* 环周长 2πr ≈ 37.7，弧占 1/4 */
.cc-statedot-arc {
  fill: none;
  stroke: var(--cc-blue);
  stroke-width: 2;
  stroke-linecap: round;
  stroke-dasharray: 9.4 28.3;
}

@keyframes cc-statedot-spin {
  to {
    transform: rotate(360deg);
  }
}

/* ── 会话目标横幅（composer 上方 dock，order 10；36px 卡片 + 圆形图标动作） ── */
.cc-goalbar {
  display: flex;
  align-items: center;
  gap: 10px;
  height: 36px;
  margin-bottom: var(--ava-space-2, 8px);
  padding: 4px 5px 4px 12px;
  border: 0.5px solid var(--cc-l2);
  border-radius: var(--ava-radius-md, 12px);
  background: var(--cc-btn-fill);
  box-shadow: var(--cc-shadow);
}

.cc-goal-glyph {
  display: inline-flex;
  flex: none;
  color: var(--cc-t3);
}

/* phase 标签带「目标」后缀（如 active → 进行中的目标） */
.cc-goal-label {
  flex: none;
  color: var(--cc-t1);
  font-size: 13px;
  line-height: 24px;
  font-weight: 500;
  white-space: nowrap;
}

.cc-goal-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  color: var(--cc-t2);
  font-size: 13px;
  line-height: 20px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cc-goalbar.is-blocked .cc-goal-text {
  color: var(--cc-danger);
}

/* 条内内联编辑输入（占满目标文本位，无边框） */
.cc-goal-input {
  flex: 1;
  min-width: 0;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--cc-t1);
  font: inherit;
  font-size: 13px;
  line-height: 20px;
  outline: none;
}

.cc-goal-actions {
  display: flex;
  align-items: center;
  flex: none;
  gap: 10px;
}

.cc-goal-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  padding: 0;
  border: 0;
  border-radius: 999px;
  background: transparent;
  color: var(--cc-t3);
  cursor: pointer;
}

.cc-goal-btn:hover {
  background: var(--cc-hover);
  color: var(--cc-t2);
}

/* ── plan 模式徽标（composer 底部工具条；点击即退出，hover 换关图标） ── */
.cc-planchip {
  display: inline-flex;
  align-items: center;
  gap: var(--ava-space-1, 4px);
  flex: none;
  height: 28px;
  padding: 0 var(--ava-space-2, 8px);
  border: 0;
  border-radius: var(--ava-radius-pill, 999px);
  background: color-mix(in srgb, var(--cc-blue) 12%, transparent);
  color: var(--cc-blue);
  font: inherit;
  font-size: 13px;
  line-height: 20px;
  font-weight: 500;
  white-space: nowrap;
  cursor: pointer;
}

.cc-planchip-ico,
.cc-planchip-close {
  display: inline-flex;
  flex: none;
}

.cc-planchip-close {
  display: none;
}

.cc-planchip:hover .cc-planchip-ico {
  display: none;
}

.cc-planchip:hover .cc-planchip-close {
  display: inline-flex;
}

.cc-planchip.is-pending {
  opacity: 0.7;
}

/* ── 斜杠命令菜单：
     combobox 模式——焦点留在 textarea，listbox 挂在滚动视口；
     表面 = 宿主浮层契约（layer-1 底 + 0.5px 发丝描边 + Panel 重浮起，lg 圆角，z 30）；
     行高 ≥34、共享高亮、图标 14/label-primary-dimmed、描述右对齐，溢出底部渐变提示。 ── */
.cc-slash {
  position: absolute;
  left: 0;
  right: 0;
  bottom: calc(100% + var(--ava-space-2, 8px));
  z-index: 30;
  display: flex;
  flex-direction: column;
  max-height: 400px;
  overflow: hidden;
  padding: var(--ava-space-1, 4px);
  border: 0.5px solid var(--cc-l1);
  border-radius: var(--ava-radius-lg, 16px);
  background: var(--ava-bg-layer-1, #ffffff);
  box-shadow: var(--ava-shadow-panel, 0 8px 24px rgba(0, 0, 0, 0.08));
}

/* 溢出下方时底部渐变提示（data-overflow-below），滚到底由 JS 移除类 */
.cc-slash.is-overflow-below::after {
  content: "";
  position: absolute;
  right: 14px;
  bottom: 4px;
  left: 4px;
  height: 16px;
  pointer-events: none;
  background: linear-gradient(to bottom, transparent, var(--ava-bg-layer-1, #ffffff));
}

.cc-slash-list {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow-y: auto;
}

.cc-slash-item {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  min-height: 34px;
  padding: 6px 8px;
  border: none;
  border-radius: var(--ava-radius-md, 12px);
  background: transparent;
  color: var(--cc-t1);
  text-align: left;
  font: inherit;
  font-size: 13px;
  line-height: 20px;
  cursor: pointer;
}

/* 单一共享高亮：指针移动与键盘共用 .on，无独立 :hover 色阶竞争 */
.cc-slash-item.on {
  background: var(--cc-hover);
}

.cc-slash-section {
  flex: none;
  min-height: 23px;
  padding: 5px 8px 2px;
  color: var(--cc-t3);
  font-size: 11px;
  font-weight: 500;
  line-height: 16px;
}

.cc-slash-section:not(:first-child) {
  margin-top: 3px;
}

.cc-slash-ico {
  display: inline-flex;
  flex: none;
  width: 14px;
  height: 14px;
  align-items: center;
  justify-content: center;
  color: var(--cc-t2);
}

.cc-slash-label {
  flex: none;
  max-width: 40%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--cc-t1);
  font-size: 13px;
  line-height: 20px;
}

/* 本地化标签旁的命令名别名：次要墨色、随长省略 */
.cc-slash-name {
  flex: none;
  max-width: 20%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--cc-t3);
  font-family: var(--cc-mono);
  font-size: 12px;
  line-height: 18px;
}

/* 描述右对齐：每行描述收在同一条右边缘，标题读成一列 */
.cc-slash-desc {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: right;
  color: var(--cc-t3);
  font-size: 12px;
  line-height: 18px;
}
`;
