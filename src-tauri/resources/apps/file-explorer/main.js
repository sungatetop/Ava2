// src/canvas-bridge.ts
var FILE_VIEWER_ID = "file-viewer";
var bus = null;
function setCanvasBus(events) {
  bus = events;
}
function openInCanvas(path, sessionId) {
  if (!bus) return;
  bus.emit("canvas.open", { target: FILE_VIEWER_ID, path, sessionId });
  bus.emit("canvas.openPath", { target: FILE_VIEWER_ID, path, sessionId });
}

// src/explorer.tsx
import { useCallback, useEffect, useState } from "react";

// src/styles.ts
var fileExplorerCss = `
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

// src/explorer.tsx
import { jsx, jsxs } from "react/jsx-runtime";
var ICON = {
  chevron: "M9 6l6 6-6 6",
  folder: "M3 7.5A1.5 1.5 0 0 1 4.5 6h5l2 2h8A1.5 1.5 0 0 1 21 9.5v8A1.5 1.5 0 0 1 19.5 19h-15A1.5 1.5 0 0 1 3 17.5v-10Z",
  file: "M6 3h8l4 4v14H6z M14 3v4h4",
  refresh: "M20 12a8 8 0 1 1-2.3-5.6 M20 4v5h-5"
};
function Icon({ d, size = 14 }) {
  return /* @__PURE__ */ jsx("svg", { width: size, height: size, viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: "1.8", strokeLinecap: "round", strokeLinejoin: "round", "aria-hidden": "true", children: /* @__PURE__ */ jsx("path", { d }) });
}
function baseName(path) {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  return parts.length > 0 ? parts[parts.length - 1] : path;
}
function FileExplorer({ agent }) {
  const [root, setRoot] = useState(null);
  const [rootSessionId, setRootSessionId] = useState(void 0);
  const [nodes, setNodes] = useState({});
  const [expanded, setExpanded] = useState(/* @__PURE__ */ new Set());
  const [rootError, setRootError] = useState(null);
  const [rootPending, setRootPending] = useState(true);
  const load = useCallback(
    (path) => {
      setNodes((prev) => ({ ...prev, [path]: { ...prev[path] ?? { dirs: [], files: [] }, loading: true, error: null } }));
      agent.listDirs(path, { files: true }).then((listing) => {
        setNodes((prev) => ({
          ...prev,
          [listing.path]: { loading: false, error: null, dirs: listing.entries, files: listing.files ?? [] }
        }));
      }).catch((e) => {
        setNodes((prev) => ({
          ...prev,
          [path]: { ...prev[path] ?? { dirs: [], files: [] }, loading: false, error: e instanceof Error ? e.message : String(e) }
        }));
      });
    },
    [agent]
  );
  useEffect(() => {
    let alive = true;
    setRootError(null);
    setRootPending(true);
    agent.listSessions().then((sessions) => {
      if (!alive) return;
      const recent = [...sessions].sort((a, b) => (b.updatedAtMs ?? 0) - (a.updatedAtMs ?? 0)).find((s) => s.workingDir);
      setRoot(recent?.workingDir ?? null);
      setRootSessionId(recent?.id);
      setRootPending(false);
      if (recent?.workingDir) load(recent.workingDir);
    }).catch((e) => {
      if (!alive) return;
      setRootError(e instanceof Error ? e.message : String(e));
      setRootPending(false);
    });
    return () => {
      alive = false;
    };
  }, [agent, load]);
  const toggle = (path) => {
    const isOpen = expanded.has(path);
    setExpanded((prev) => {
      const next = new Set(prev);
      if (isOpen) next.delete(path);
      else next.add(path);
      return next;
    });
    if (!isOpen && !nodes[path] && !rootError) load(path);
  };
  const renderDir = (entry, depth) => {
    const open = expanded.has(entry.path);
    const state = nodes[entry.path];
    return /* @__PURE__ */ jsxs("div", { children: [
      /* @__PURE__ */ jsxs("button", { className: "fe-row", style: { paddingLeft: 8 + depth * 18 }, onClick: () => toggle(entry.path), title: entry.path, children: [
        /* @__PURE__ */ jsx("span", { className: "fe-chev" + (open ? " open" : ""), children: /* @__PURE__ */ jsx(Icon, { d: ICON.chevron, size: 10 }) }),
        /* @__PURE__ */ jsx("span", { className: "fe-icon", children: /* @__PURE__ */ jsx(Icon, { d: ICON.folder, size: 13 }) }),
        /* @__PURE__ */ jsx("span", { className: "fe-name", children: entry.name })
      ] }),
      open && /* @__PURE__ */ jsxs("div", { children: [
        state?.loading && /* @__PURE__ */ jsx("div", { className: "fe-state", style: { paddingLeft: 8 + (depth + 1) * 18 }, children: "\u8BFB\u53D6\u4E2D\u2026" }),
        state?.error && /* @__PURE__ */ jsx("div", { className: "fe-state err", style: { paddingLeft: 8 + (depth + 1) * 18 }, children: state.error }),
        state && !state.loading && !state.error && state.dirs.length === 0 && state.files.length === 0 && /* @__PURE__ */ jsx("div", { className: "fe-state", style: { paddingLeft: 8 + (depth + 1) * 18 }, children: "\u7A7A\u76EE\u5F55" }),
        state?.dirs.map((child) => renderDir(child, depth + 1)),
        state?.files.map((file) => /* @__PURE__ */ jsxs(
          "button",
          {
            className: "fe-row",
            style: { paddingLeft: 8 + (depth + 1) * 18 },
            title: `${file.path}
\u53CC\u51FB\u5728\u753B\u5E03\u9884\u89C8`,
            onDoubleClick: () => openInCanvas(file.path, rootSessionId),
            children: [
              /* @__PURE__ */ jsx("span", { className: "fe-chev ghost" }),
              /* @__PURE__ */ jsx("span", { className: "fe-icon", children: /* @__PURE__ */ jsx(Icon, { d: ICON.file, size: 13 }) }),
              /* @__PURE__ */ jsx("span", { className: "fe-name file", children: file.name })
            ]
          },
          file.path
        ))
      ] })
    ] }, entry.path);
  };
  const rootState = root ? nodes[root] : void 0;
  return /* @__PURE__ */ jsxs("div", { className: "fe-root", children: [
    /* @__PURE__ */ jsx("style", { children: fileExplorerCss }),
    /* @__PURE__ */ jsxs("div", { className: "fe-head", children: [
      /* @__PURE__ */ jsx("span", { className: "fe-title", children: "\u6587\u4EF6" }),
      root && /* @__PURE__ */ jsx("span", { className: "fe-root-name", title: root, children: baseName(root) }),
      /* @__PURE__ */ jsx(
        "button",
        {
          className: "fe-btn",
          title: "\u5237\u65B0\u76EE\u5F55",
          disabled: !root,
          onClick: () => root && load(root),
          children: /* @__PURE__ */ jsx(Icon, { d: ICON.refresh, size: 13 })
        }
      )
    ] }),
    /* @__PURE__ */ jsxs("div", { className: "fe-body", children: [
      rootError && /* @__PURE__ */ jsxs("div", { className: "fe-state err", children: [
        "\u4F1A\u8BDD\u5217\u8868\u8BFB\u53D6\u5931\u8D25\uFF1A",
        rootError
      ] }),
      !rootError && rootPending && /* @__PURE__ */ jsx("div", { className: "fe-state", children: "\u6B63\u5728\u5B9A\u4F4D\u5DE5\u4F5C\u533A\u2026" }),
      !rootError && !rootPending && !root && /* @__PURE__ */ jsx("div", { className: "fe-state", children: "\u6682\u65E0\u4F1A\u8BDD\uFF0C\u65E0\u6CD5\u786E\u5B9A\u5DE5\u4F5C\u533A" }),
      root && /* @__PURE__ */ jsxs("div", { className: "fe-tree", children: [
        rootState?.loading && /* @__PURE__ */ jsx("div", { className: "fe-state", children: "\u8BFB\u53D6\u4E2D\u2026" }),
        rootState?.error && /* @__PURE__ */ jsx("div", { className: "fe-state err", children: rootState.error }),
        rootState?.dirs.map((child) => renderDir(child, 0)),
        rootState?.files.map((file) => /* @__PURE__ */ jsxs(
          "button",
          {
            className: "fe-row",
            style: { paddingLeft: 8 },
            title: `${file.path}
\u53CC\u51FB\u5728\u753B\u5E03\u9884\u89C8`,
            onDoubleClick: () => openInCanvas(file.path, rootSessionId),
            children: [
              /* @__PURE__ */ jsx("span", { className: "fe-chev ghost" }),
              /* @__PURE__ */ jsx("span", { className: "fe-icon", children: /* @__PURE__ */ jsx(Icon, { d: ICON.file, size: 13 }) }),
              /* @__PURE__ */ jsx("span", { className: "fe-name file", children: file.name })
            ]
          },
          file.path
        ))
      ] })
    ] })
  ] });
}

// src/main.tsx
import { jsx as jsx2 } from "react/jsx-runtime";
function setup(ctx) {
  setCanvasBus(ctx.events);
  const disposers = [
    ctx.slots.inject("panel.canvas", {
      // 全局画布面板：viewId "*" 使其跟随所有画布视图（含 canvas:home），独立于文件预览
      order: 6,
      viewId: "*",
      title: "\u6587\u4EF6",
      // folder-tree（lucide 同源）：树形目录，多路径组合（应用市场图标）
      icon: {
        d: [
          "M20 10a1 1 0 0 0 1-1V6a1 1 0 0 0-1-1h-2.5a1 1 0 0 1-.8-.4l-.9-1.2A1 1 0 0 0 15 3h-2a1 1 0 0 0-1 1v5a1 1 0 0 0 1 1Z",
          "M20 21a1 1 0 0 0 1-1v-3a1 1 0 0 0-1-1h-2.9a1 1 0 0 1-.88-.55l-.42-.85a1 1 0 0 0-.92-.6H13a1 1 0 0 0-1 1v5a1 1 0 0 0 1 1Z",
          "M3 5a2 2 0 0 0 2 2h3",
          "M3 3v13a2 2 0 0 0 2 2h3"
        ]
      },
      component: async () => ({
        default: () => /* @__PURE__ */ jsx2(FileExplorer, { agent: ctx.agent })
      })
    })
  ];
  return {
    dispose() {
      for (const d of disposers) d.dispose();
    }
  };
}
export {
  setup
};
