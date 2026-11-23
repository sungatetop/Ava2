// file-explorer 样式：全部走宿主 design token（--ava-*），选择器限域 .fe-root。
// 形态：38px 头部固定、行高 28px、每级缩进 18px；折叠由宿主 tab 栏按钮控制。
export const fileExplorerCss = `
.fe-root {
  --fe-t1: var(--ava-label-primary, #0f1115);
  --fe-t2: var(--ava-label-secondary, #61666b);
  --fe-t3: var(--ava-label-tertiary, #81858c);
  --fe-l2: var(--ava-border-l2, rgba(0, 0, 0, 0.1));
  --fe-hover: var(--ava-interactive-bg-hover, rgba(38, 49, 72, 0.06));
  --fe-danger: var(--ava-state-danger, #ec1313);

  display: flex;
  flex-direction: column;
  flex: 1;
  height: 100%;
  min-height: 0;
  color: var(--fe-t1);
  font-size: 12px;
  line-height: 18px;
}

.fe-head {
  display: flex;
  align-items: center;
  gap: var(--ava-space-1, 4px);
  flex: none;
  height: 38px;
  padding: 0 var(--ava-space-2, 8px);
  min-width: 0;
  border-bottom: 0.5px solid var(--fe-l2);
}

.fe-btn {
  display: inline-grid;
  place-items: center;
  flex: none;
  width: 22px;
  height: 22px;
  padding: 0;
  border: none;
  border-radius: var(--ava-radius-sm, 8px);
  background: transparent;
  color: var(--fe-t3);
  cursor: pointer;
}

.fe-btn:hover:not(:disabled) {
  background: var(--fe-hover);
  color: var(--fe-t1);
}

.fe-btn:disabled {
  opacity: 0.45;
  cursor: default;
}

.fe-title {
  flex: none;
  color: var(--fe-t2);
  font-weight: 600;
}

.fe-root-name {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--fe-t3);
  font-family: var(--ava-font-mono, Consolas, monospace);
  font-size: 11px;
}

.fe-body {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.fe-tree {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding-bottom: var(--ava-space-2, 8px);
}

.fe-row {
  display: flex;
  align-items: center;
  gap: var(--ava-space-1, 4px);
  width: 100%;
  height: 28px;
  padding: 0 var(--ava-space-2, 8px);
  border: none;
  background: transparent;
  color: var(--fe-t1);
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.fe-row:hover {
  background: var(--fe-hover);
}

.fe-row.selected {
  background: var(--fe-hover);
}

.fe-row .fe-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.fe-row .fe-name.file {
  color: var(--fe-t2);
}

.fe-icon {
  display: inline-grid;
  place-items: center;
  flex: none;
  color: var(--fe-t3);
}

.fe-chev {
  display: inline-grid;
  place-items: center;
  flex: none;
  width: 12px;
  color: var(--fe-t3);
  transition: transform 0.15s ease;
}

.fe-chev.open {
  transform: rotate(90deg);
}

.fe-chev.ghost {
  visibility: hidden;
}

.fe-state {
  padding: var(--ava-space-1, 4px) var(--ava-space-2, 8px);
  color: var(--fe-t3);
}

.fe-state.err {
  color: var(--fe-danger);
  word-break: break-all;
}
`;
