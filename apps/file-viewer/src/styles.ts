// file-viewer 样式：全部走宿主 design token（--ava-*），选择器限域 .fv-root。
export const fileViewerCss = `
.fv-root {
  --fv-t1: var(--ava-label-primary, #0f1115);
  --fv-t2: var(--ava-label-secondary, #61666b);
  --fv-t3: var(--ava-label-tertiary, #81858c);
  --fv-l2: var(--ava-border-l2, rgba(0, 0, 0, 0.1));
  --fv-hover: var(--ava-interactive-bg-hover, rgba(38, 49, 72, 0.06));
  --fv-danger: var(--ava-state-danger, #ec1313);
  --fv-bg: var(--ava-surface-elevated, #ffffff);
  --fv-font: var(--ava-font-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC",
    "Microsoft YaHei", system-ui, sans-serif);
  --fv-mono: var(--ava-font-mono, "SF Mono", "JetBrains Mono", "Fira Code", Consolas, monospace);

  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  color: var(--fv-t1);
  font-family: var(--fv-font);
  font-size: 13px;
  line-height: 20px;
}

.fv-head {
  display: flex;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  padding: var(--ava-space-2, 8px) var(--ava-space-3, 12px);
  border-bottom: 0.5px solid var(--fv-l2);
  min-width: 0;
}

/* 多文件标签条：占满剩余宽度可横向滚动；完整路径仅在 hover tab（title 提示）时显示 */
.fv-tabs {
  display: flex;
  align-items: center;
  gap: var(--ava-space-1, 4px);
  flex: 1 1 auto;
  min-width: 0;
  overflow-x: auto;
  padding-bottom: 2px;
}

.fv-tabs-empty {
  color: var(--fv-t3);
  font-size: 12px;
  white-space: nowrap;
}

.fv-tab {
  display: inline-flex;
  align-items: center;
  gap: var(--ava-space-1, 4px);
  flex: none;
  max-width: 180px;
  height: 24px;
  padding: 0 var(--ava-space-1, 4px) 0 var(--ava-space-2, 8px);
  border: 0.5px solid var(--fv-l2);
  border-radius: var(--ava-radius-sm, 8px);
  background: transparent;
  color: var(--fv-t2);
  font-size: 12px;
  cursor: pointer;
}

.fv-tab.on {
  background: var(--ava-nav-active, rgba(65, 118, 230, 0.1));
  color: var(--fv-t1);
}

.fv-tab-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.fv-tab-close {
  display: inline-grid;
  place-items: center;
  flex: none;
  width: 16px;
  height: 16px;
  padding: 0;
  border: none;
  border-radius: var(--ava-radius-xs, 4px);
  background: transparent;
  color: var(--fv-t3);
  cursor: pointer;
}

.fv-tab-close:hover {
  background: var(--fv-hover);
  color: var(--fv-t1);
}

.fv-btn {
  flex: 0 0 auto;
  height: 26px;
  padding: 0 var(--ava-space-3, 12px);
  border: 0.5px solid var(--fv-l2);
  border-radius: var(--ava-radius-md, 8px);
  background: var(--fv-bg);
  color: var(--fv-t1);
  font-family: inherit;
  font-size: 12px;
  cursor: pointer;
}

.fv-btn:hover:not(:disabled) {
  background: var(--fv-hover);
}

.fv-btn:disabled {
  color: var(--fv-t3);
  cursor: default;
}

.fv-body {
  flex: 1 1 auto;
  min-height: 0;
  overflow: auto;
  position: relative;
}

.fv-text {
  margin: 0;
  padding: var(--ava-space-3, 12px);
  font-family: var(--fv-mono);
  font-size: 12px;
  line-height: 20px;
  white-space: pre-wrap;
  word-break: break-word;
}

.fv-markdown {
  padding: var(--ava-space-3, 12px) var(--ava-space-4, 16px);
  font-size: 13px;
  line-height: 22px;
  word-break: break-word;
}

.fv-markdown > :first-child {
  margin-top: 0;
}

.fv-markdown > :last-child {
  margin-bottom: 0;
}

.fv-markdown h1,
.fv-markdown h2,
.fv-markdown h3,
.fv-markdown h4 {
  margin: 16px 0 8px;
  line-height: 1.4;
}

.fv-markdown h1 {
  font-size: 18px;
  padding-bottom: 6px;
  border-bottom: 0.5px solid var(--fv-l2);
}

.fv-markdown h2 {
  font-size: 16px;
}

.fv-markdown h3,
.fv-markdown h4 {
  font-size: 14px;
}

.fv-markdown p {
  margin: 8px 0;
}

.fv-markdown a {
  color: var(--ava-state-business-primary, #4176e6);
  text-decoration: none;
}

.fv-markdown a:hover {
  text-decoration: underline;
}

.fv-markdown ul,
.fv-markdown ol {
  margin: 8px 0;
  padding-left: 22px;
}

.fv-markdown li {
  margin: 3px 0;
}

.fv-markdown code {
  padding: 1px 4px;
  border-radius: var(--ava-radius-xs, 4px);
  background: var(--ava-surface-subtle, rgba(0, 0, 0, 0.06));
  font-family: var(--fv-mono);
  font-size: 12px;
}

.fv-markdown pre {
  margin: 10px 0;
  padding: 10px 12px;
  border-radius: var(--ava-radius-md, 8px);
  background: var(--ava-surface-subtle, rgba(0, 0, 0, 0.06));
  overflow-x: auto;
}

.fv-markdown pre code {
  padding: 0;
  background: none;
}

.fv-markdown blockquote {
  margin: 10px 0;
  padding: 4px 12px;
  border-left: 3px solid var(--fv-l2);
  color: var(--fv-t2);
}

.fv-markdown table {
  margin: 10px 0;
  border-collapse: collapse;
  font-size: 12px;
}

.fv-markdown th,
.fv-markdown td {
  padding: 6px 10px;
  border: 0.5px solid var(--fv-l2);
}

.fv-markdown th {
  background: var(--ava-surface-subtle, rgba(0, 0, 0, 0.06));
  font-weight: 600;
}

.fv-markdown hr {
  margin: 14px 0;
  border: none;
  border-top: 0.5px solid var(--fv-l2);
}

.fv-markdown img {
  max-width: 100%;
  border-radius: var(--ava-radius-md, 8px);
}

/* HTML 预览：iframe 白底，绝对定位铺满 .fv-body（适配 viewer 高宽，不受百分比高度解析影响）；
   sandbox="allow-scripts"（opaque origin，脚本可运行但无法访问宿主），对齐 ava-app canvas */
.fv-iframe {
  position: absolute;
  inset: 0;
  display: block;
  width: 100%;
  height: 100%;
  border: none;
  background: #fff;
}

/* 媒体预览（视频/音频）：占满预览区并居中播放控件，视频限在视区内不溢出 */
.fv-media {
  display: flex;
  justify-content: center;
  align-items: center;
  height: 100%;
  padding: var(--ava-space-4, 16px);
}

.fv-video {
  max-width: 100%;
  max-height: 100%;
  border-radius: var(--ava-radius-md, 8px);
  background: #000;
}

.fv-audio {
  display: flex;
  justify-content: center;
  align-items: center;
  padding: var(--ava-space-4, 16px);
}

.fv-audio audio {
  width: min(480px, 100%);
}

/* 代码/纯文本预览：行号列 + 内容列（轻量只读代码视图，对齐 ava-app 只读编辑器观感） */
.fv-code {
  padding: var(--ava-space-3, 12px) 0;
  font-family: var(--fv-mono);
  font-size: 12px;
  line-height: 20px;
}

.fv-code-line {
  display: flex;
}

.fv-code-no {
  flex: none;
  width: 48px;
  padding-right: var(--ava-space-2, 8px);
  text-align: right;
  color: var(--fv-t3);
  user-select: none;
}

.fv-code-text {
  flex: 1 1 auto;
  min-width: 0;
  padding: 0 var(--ava-space-3, 12px);
  white-space: pre-wrap;
  word-break: break-word;
}

/* Office 文档渲染容器（mammoth docx / SheetJS xlsx 输出的 HTML） */
.fv-doc {
  padding: var(--ava-space-3, 12px) var(--ava-space-4, 16px);
  font-size: 13px;
  line-height: 22px;
  word-break: break-word;
}

.fv-doc > :first-child {
  margin-top: 0;
}

.fv-doc > :last-child {
  margin-bottom: 0;
}

.fv-doc p {
  margin: 8px 0;
}

.fv-doc h1,
.fv-doc h2,
.fv-doc h3,
.fv-doc h4 {
  margin: 16px 0 8px;
  line-height: 1.4;
}

.fv-doc table {
  margin: 10px 0;
  border-collapse: collapse;
  font-size: 12px;
}

.fv-doc th,
.fv-doc td {
  padding: 6px 10px;
  border: 0.5px solid var(--fv-l2);
}

.fv-doc th {
  background: var(--ava-surface-subtle, rgba(0, 0, 0, 0.06));
  font-weight: 600;
}

.fv-doc img {
  max-width: 100%;
  border-radius: var(--ava-radius-md, 8px);
}

/* pptx 幻灯片文本提取渲染 */
.fv-pptx {
  padding: var(--ava-space-3, 12px);
  display: flex;
  flex-direction: column;
  gap: var(--ava-space-3, 12px);
}

.fv-slide {
  border: 0.5px solid var(--fv-l2);
  border-radius: var(--ava-radius-md, 8px);
  overflow: hidden;
}

.fv-slide-no {
  padding: 4px var(--ava-space-3, 12px);
  border-bottom: 0.5px solid var(--fv-l2);
  color: var(--fv-t3);
  font-size: 12px;
}

.fv-slide-text {
  margin: 0;
  padding: var(--ava-space-3, 12px);
  white-space: pre-wrap;
  word-break: break-word;
  font-family: var(--fv-mono);
  font-size: 12px;
  line-height: 20px;
}

.fv-image {
  display: flex;
  justify-content: center;
  padding: var(--ava-space-4, 16px);
}

.fv-image img {
  max-width: 100%;
  height: auto;
  border-radius: var(--ava-radius-md, 8px);
}

.fv-empty,
.fv-error {
  padding: var(--ava-space-4, 16px);
  color: var(--fv-t2);
}

/* 空态：图标底座 + 主/副文案，视觉居中 */
.fv-empty {
  margin-top: 18vh;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  text-align: center;
  font-size: 13px;
}

.fv-empty-ico {
  display: grid;
  place-items: center;
  width: 44px;
  height: 44px;
  border-radius: var(--ava-radius-md, 8px);
  background: var(--fv-hover);
  color: var(--fv-t3);
}

.fv-empty-title {
  color: var(--fv-t1);
  font-weight: 500;
}

.fv-error {
  color: var(--fv-danger);
}
`;
