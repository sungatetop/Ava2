var __create = Object.create;
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __getProtoOf = Object.getPrototypeOf;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __esm = (fn, res) => function __init() {
  return fn && (res = (0, fn[__getOwnPropNames(fn)[0]])(fn = 0)), res;
};
var __commonJS = (cb, mod) => function __require() {
  return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};
var __export = (target, all2) => {
  for (var name2 in all2)
    __defProp(target, name2, { get: all2[name2], enumerable: true });
};
var __copyProps = (to, from, except, desc) => {
  if (from && typeof from === "object" || typeof from === "function") {
    for (let key of __getOwnPropNames(from))
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
  }
  return to;
};
var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(
  // If the importer is in node compatibility mode or this is not an ESM
  // file that has been converted to a CommonJS file using a Babel-
  // compatible transform (i.e. "__esModule" has not been set), then set
  // "default" to the CommonJS "module.exports" for node compatibility.
  isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", { value: mod, enumerable: true }) : target,
  mod
));

// node_modules/extend/index.js
var require_extend = __commonJS({
  "node_modules/extend/index.js"(exports, module) {
    "use strict";
    var hasOwn = Object.prototype.hasOwnProperty;
    var toStr = Object.prototype.toString;
    var defineProperty2 = Object.defineProperty;
    var gOPD = Object.getOwnPropertyDescriptor;
    var isArray = function isArray2(arr) {
      if (typeof Array.isArray === "function") {
        return Array.isArray(arr);
      }
      return toStr.call(arr) === "[object Array]";
    };
    var isPlainObject2 = function isPlainObject3(obj) {
      if (!obj || toStr.call(obj) !== "[object Object]") {
        return false;
      }
      var hasOwnConstructor = hasOwn.call(obj, "constructor");
      var hasIsPrototypeOf = obj.constructor && obj.constructor.prototype && hasOwn.call(obj.constructor.prototype, "isPrototypeOf");
      if (obj.constructor && !hasOwnConstructor && !hasIsPrototypeOf) {
        return false;
      }
      var key;
      for (key in obj) {
      }
      return typeof key === "undefined" || hasOwn.call(obj, key);
    };
    var setProperty = function setProperty2(target, options) {
      if (defineProperty2 && options.name === "__proto__") {
        defineProperty2(target, options.name, {
          enumerable: true,
          configurable: true,
          value: options.newValue,
          writable: true
        });
      } else {
        target[options.name] = options.newValue;
      }
    };
    var getProperty = function getProperty2(obj, name2) {
      if (name2 === "__proto__") {
        if (!hasOwn.call(obj, name2)) {
          return void 0;
        } else if (gOPD) {
          return gOPD(obj, name2).value;
        }
      }
      return obj[name2];
    };
    module.exports = function extend2() {
      var options, name2, src, copy, copyIsArray, clone;
      var target = arguments[0];
      var i = 1;
      var length = arguments.length;
      var deep = false;
      if (typeof target === "boolean") {
        deep = target;
        target = arguments[1] || {};
        i = 2;
      }
      if (target == null || typeof target !== "object" && typeof target !== "function") {
        target = {};
      }
      for (; i < length; ++i) {
        options = arguments[i];
        if (options != null) {
          for (name2 in options) {
            src = getProperty(target, name2);
            copy = getProperty(options, name2);
            if (target !== copy) {
              if (deep && copy && (isPlainObject2(copy) || (copyIsArray = isArray(copy)))) {
                if (copyIsArray) {
                  copyIsArray = false;
                  clone = src && isArray(src) ? src : [];
                } else {
                  clone = src && isPlainObject2(src) ? src : {};
                }
                setProperty(target, { name: name2, newValue: extend2(deep, clone, copy) });
              } else if (typeof copy !== "undefined") {
                setProperty(target, { name: name2, newValue: copy });
              }
            }
          }
        }
      }
      return target;
    };
  }
});

// src/settings.tsx
var settings_exports = {};
__export(settings_exports, {
  default: () => SettingsPanel
});
import { useState as useState9 } from "react";
import { jsx as jsx9, jsxs as jsxs8 } from "react/jsx-runtime";
function readConfig() {
  try {
    return JSON.parse(localStorage.getItem(CFG_KEY) || "{}");
  } catch {
    return {};
  }
}
function SettingsPanel() {
  const [cfg, setCfg] = useState9(readConfig);
  const save = (patch2) => {
    const next = { ...readConfig(), ...patch2 };
    localStorage.setItem(CFG_KEY, JSON.stringify(next));
    setCfg(next);
  };
  return /* @__PURE__ */ jsxs8("div", { style: { display: "flex", flexDirection: "column", gap: 12, padding: "4px 0" }, children: [
    /* @__PURE__ */ jsxs8("label", { style: row, children: [
      /* @__PURE__ */ jsx9("span", { style: { width: 64 }, children: "\u6635\u79F0" }),
      /* @__PURE__ */ jsx9(
        "input",
        {
          style: { ...input, flex: 1 },
          value: cfg.nickname ?? "",
          placeholder: "\u7ED9\u81EA\u5DF1\u8D77\u4E2A\u540D\u5B57",
          onChange: (e) => save({ nickname: e.target.value || void 0 })
        }
      )
    ] }),
    /* @__PURE__ */ jsxs8("label", { style: row, children: [
      /* @__PURE__ */ jsx9("span", { style: { width: 64 }, children: "\u5F3A\u8C03\u8272" }),
      /* @__PURE__ */ jsxs8(
        "select",
        {
          style: { ...input, flex: 1 },
          value: cfg.accent ?? "",
          onChange: (e) => save({ accent: e.target.value || void 0 }),
          children: [
            /* @__PURE__ */ jsx9("option", { value: "", children: "\u8DDF\u968F\u5BBF\u4E3B" }),
            /* @__PURE__ */ jsx9("option", { value: "blue", children: "\u84DD" }),
            /* @__PURE__ */ jsx9("option", { value: "green", children: "\u7EFF" }),
            /* @__PURE__ */ jsx9("option", { value: "orange", children: "\u6A59" })
          ]
        }
      )
    ] }),
    /* @__PURE__ */ jsx9("div", { style: { fontSize: 12, color: "var(--ava-label-tertiary)" }, children: "\u914D\u7F6E\u7531\u672C App \u81EA\u884C\u5B58\u50A8\uFF08localStorage\uFF09\uFF0C\u5373\u65F6\u751F\u6548\uFF1B\u5BBF\u4E3B\u4E0D\u53C2\u4E0E\u5B58\u50A8\u4E0E\u6821\u9A8C\u3002" })
  ] });
}
var CFG_KEY, row, input;
var init_settings = __esm({
  "src/settings.tsx"() {
    "use strict";
    CFG_KEY = "chatapp:cfg";
    row = { display: "flex", alignItems: "center", gap: 10, fontSize: 13 };
    input = {
      padding: "5px 10px",
      border: "0.5px solid var(--ava-border-l2)",
      borderRadius: 8,
      background: "var(--ava-bg-base)",
      color: "var(--ava-label-primary)",
      font: "inherit"
    };
  }
});

// src/canvas-bridge.ts
var FILE_VIEWER_ID = "file-viewer";
var bus = null;
function setCanvasBus(events) {
  bus = events;
}
function openInCanvas(path2, sessionId) {
  if (!bus) return;
  const payload = { target: FILE_VIEWER_ID, path: path2, sessionId };
  bus.emit("canvas.open", payload);
  bus.emit("canvas.openPath", payload);
}

// src/chat-view.tsx
import { Fragment as Fragment6, useEffect as useEffect6, useRef as useRef5, useState as useState7 } from "react";

// src/composer-chips.tsx
import { useEffect as useEffect2, useRef as useRef2, useState as useState2 } from "react";

// src/popover.tsx
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { jsx } from "react/jsx-runtime";
var GAP = 8;
var EDGE = 8;
var MIN_SIDE_SPACE = 180;
function place(anchor, side, width) {
  const r = anchor.getBoundingClientRect();
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  const w = Math.min(width, vw - EDGE * 2);
  const left = Math.min(Math.max(EDGE, r.left), vw - w - EDGE);
  const above = r.top - GAP - EDGE;
  const below = vh - r.bottom - GAP - EDGE;
  const up = side === "top" ? above >= MIN_SIDE_SPACE || above >= below : below < MIN_SIDE_SPACE && above > below;
  return up ? { left, width: w, bottom: vh - r.top + GAP, maxHeight: Math.max(MIN_SIDE_SPACE, above) } : { left, width: w, top: r.bottom + GAP, maxHeight: Math.max(MIN_SIDE_SPACE, below) };
}
function Popover({ anchor, open, onClose, side = "top", width = 280, children }) {
  const panelRef = useRef(null);
  const [, bump] = useState(0);
  useLayoutEffect(() => {
    if (!open || anchor === null) return;
    const follow = () => bump((n) => n + 1);
    window.addEventListener("resize", follow);
    window.addEventListener("scroll", follow, true);
    return () => {
      window.removeEventListener("resize", follow);
      window.removeEventListener("scroll", follow, true);
    };
  }, [open, anchor, side, width]);
  useEffect(() => {
    if (!open) return;
    const onDown = (e) => {
      const target = e.target;
      if (panelRef.current?.contains(target)) return;
      if (anchor?.contains(target)) return;
      onClose();
    };
    const onKey = (e) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open, anchor, onClose]);
  if (!open || anchor === null) return null;
  return createPortal(
    /* @__PURE__ */ jsx("div", { ref: panelRef, className: "cc-pop cc-menu", style: place(anchor, side, width), children }),
    document.body
  );
}

// src/composer-chips.tsx
import { Fragment, jsx as jsx2, jsxs } from "react/jsx-runtime";
var APPROVALS = [
  { value: "workspace-write", label: "\u5DE5\u4F5C\u533A\u5199", hint: "\u53EA\u8BFB\u4E0E\u5DE5\u4F5C\u533A\u5185\u5199\u76F4\u901A\uFF1B\u8D8A\u754C\u5199\u3001\u7834\u574F\u6027\u547D\u4EE4\u3001\u7F51\u7EDC\u8BBF\u95EE\u9700\u5BA1\u6279" },
  { value: "auto", label: "\u81EA\u52A8", hint: "\u5E38\u89C4\u5DE5\u5177\u8C03\u7528\u81EA\u52A8\u653E\u884C" },
  { value: "danger-full-access", label: "\u5168\u653E\u884C", hint: "\u8DF3\u8FC7\u6240\u6709\u5BA1\u6279\uFF0C\u8C28\u614E\u4F7F\u7528" }
];
function shortDir(p) {
  const parts = p.split(/[\\/]/).filter(Boolean);
  return parts.length > 2 ? `${parts[0]}/\u2026/${parts[parts.length - 1]}` : parts.join("/");
}
function Chevron({ open }) {
  return /* @__PURE__ */ jsx2(
    "svg",
    {
      className: "cc-chip-chevron" + (open ? " open" : ""),
      width: "10",
      height: "10",
      viewBox: "0 0 24 24",
      fill: "none",
      stroke: "currentColor",
      strokeWidth: "2.5",
      strokeLinecap: "round",
      strokeLinejoin: "round",
      "aria-hidden": "true",
      children: /* @__PURE__ */ jsx2("path", { d: "M6 9l6 6 6-6" })
    }
  );
}
function CheckIcon() {
  return /* @__PURE__ */ jsx2("svg", { width: "14", height: "14", viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: "2.2", strokeLinecap: "round", strokeLinejoin: "round", "aria-hidden": "true", children: /* @__PURE__ */ jsx2("path", { d: "M20 6 9 17l-5-5" }) });
}
function FolderIcon() {
  return /* @__PURE__ */ jsx2("svg", { width: "16", height: "16", viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: "1.7", strokeLinecap: "round", strokeLinejoin: "round", "aria-hidden": "true", children: /* @__PURE__ */ jsx2("path", { d: "M3 7.5A1.5 1.5 0 0 1 4.5 6h5l2 2h8A1.5 1.5 0 0 1 21 9.5v8A1.5 1.5 0 0 1 19.5 19h-15A1.5 1.5 0 0 1 3 17.5v-10Z" }) });
}
function ApprovalIcon({ preset }) {
  const common = { width: 14, height: 14, viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: 2, strokeLinecap: "round", strokeLinejoin: "round", "aria-hidden": true };
  if (preset === "auto") {
    return /* @__PURE__ */ jsx2("svg", { ...common, children: /* @__PURE__ */ jsx2("path", { d: "M13 2 4.5 13.5H11l-1 8.5 8.5-11.5H12l1-8.5Z" }) });
  }
  if (preset === "danger-full-access") {
    return /* @__PURE__ */ jsxs("svg", { ...common, children: [
      /* @__PURE__ */ jsx2("path", { d: "M12 3 2.5 20h19L12 3Z" }),
      /* @__PURE__ */ jsx2("path", { d: "M12 9v5M12 17.5v.01" })
    ] });
  }
  return /* @__PURE__ */ jsxs("svg", { ...common, children: [
    /* @__PURE__ */ jsx2("path", { d: "M4 20h4l10.5-10.5a2.1 2.1 0 0 0-3-3L5 17v3Z" }),
    /* @__PURE__ */ jsx2("path", { d: "M14 6.5 17.5 10" })
  ] });
}
function ModelIcon() {
  return /* @__PURE__ */ jsxs("svg", { width: "14", height: "14", viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: "1.8", strokeLinecap: "round", strokeLinejoin: "round", "aria-hidden": "true", children: [
    /* @__PURE__ */ jsx2("path", { d: "M12 3 4 7.5v9L12 21l8-4.5v-9L12 3Z" }),
    /* @__PURE__ */ jsx2("path", { d: "M4 7.5 12 12l8-4.5M12 12v9" })
  ] });
}
function MenuList({ children, autoFocusItem = true }) {
  const ref = useRef2(null);
  useEffect2(() => {
    if (!autoFocusItem) return;
    const root4 = ref.current;
    const checked = root4?.querySelector('[aria-checked="true"]:not(:disabled)');
    (checked ?? root4?.querySelector('[role="menuitem"]:not(:disabled)'))?.focus();
  }, [autoFocusItem]);
  useEffect2(() => {
    const onKey = (e) => {
      if (e.key === "Enter") {
        const item = document.activeElement;
        if (item?.getAttribute("role")?.startsWith("menuitem")) {
          e.preventDefault();
          item.click();
        }
        return;
      }
      if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
      const items = Array.from(
        ref.current?.querySelectorAll('[role="menuitem"]:not(:disabled), [role="menuitemradio"]:not(:disabled)') ?? []
      );
      if (items.length === 0) return;
      e.preventDefault();
      const at = items.findIndex((item) => item === document.activeElement);
      const next = at === -1 ? e.key === "ArrowDown" ? 0 : items.length - 1 : (at + (e.key === "ArrowDown" ? 1 : -1) + items.length) % items.length;
      items[next]?.focus();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);
  return /* @__PURE__ */ jsx2("div", { className: "cc-menu-list", ref, role: "menu", children });
}
function WorkspaceChip({ current, workspaces, disabled, onPick, onBrowse }) {
  const [open, setOpen] = useState2(false);
  const [anchor, setAnchor] = useState2(null);
  const label = current ? shortDir(current) : "\u9009\u62E9\u76EE\u5F55";
  const picked = (dir) => {
    setOpen(false);
    onPick(dir);
  };
  return /* @__PURE__ */ jsxs(Fragment, { children: [
    /* @__PURE__ */ jsxs(
      "button",
      {
        ref: setAnchor,
        type: "button",
        className: "cc-chip-btn",
        "aria-haspopup": "menu",
        "aria-expanded": open,
        title: current ? `\u5F53\u524D\u5DE5\u4F5C\u533A\uFF1A${current}` : "\u9009\u62E9\u5DE5\u4F5C\u76EE\u5F55",
        disabled,
        onClick: () => setOpen((v) => !v),
        children: [
          /* @__PURE__ */ jsx2("span", { className: "cc-chip-icon", children: /* @__PURE__ */ jsx2(FolderIcon, {}) }),
          /* @__PURE__ */ jsx2("span", { className: "cc-chip-text", children: label }),
          /* @__PURE__ */ jsx2(Chevron, { open })
        ]
      }
    ),
    /* @__PURE__ */ jsxs(Popover, { anchor, open, onClose: () => setOpen(false), width: 340, children: [
      /* @__PURE__ */ jsx2("div", { className: "cc-menu-path", title: current, children: current ?? "\u5C1A\u672A\u9009\u62E9\u5DE5\u4F5C\u533A" }),
      workspaces.length > 0 && /* @__PURE__ */ jsxs(MenuList, { children: [
        /* @__PURE__ */ jsx2("div", { className: "cc-menu-group-head", children: "\u6700\u8FD1\u4F7F\u7528" }),
        workspaces.map((dir) => /* @__PURE__ */ jsxs(
          "button",
          {
            type: "button",
            role: "menuitemradio",
            "aria-checked": dir === current,
            className: "cc-menu-item",
            title: dir,
            onClick: () => picked(dir),
            children: [
              /* @__PURE__ */ jsx2("span", { className: "cc-menu-item-icon", children: /* @__PURE__ */ jsx2(FolderIcon, {}) }),
              /* @__PURE__ */ jsx2("span", { className: "cc-menu-item-text", children: shortDir(dir) }),
              /* @__PURE__ */ jsx2("span", { className: "cc-menu-check", children: dir === current && /* @__PURE__ */ jsx2(CheckIcon, {}) })
            ]
          },
          dir
        ))
      ] }),
      /* @__PURE__ */ jsx2("div", { className: "cc-menu-sep" }),
      /* @__PURE__ */ jsxs(
        "button",
        {
          type: "button",
          role: "menuitem",
          className: "cc-menu-item",
          onClick: () => {
            setOpen(false);
            onBrowse();
          },
          children: [
            /* @__PURE__ */ jsx2("span", { className: "cc-menu-item-icon", children: /* @__PURE__ */ jsx2(FolderIcon, {}) }),
            /* @__PURE__ */ jsx2("span", { className: "cc-menu-item-text", children: "\u9009\u62E9\u5176\u4ED6\u76EE\u5F55\u2026" })
          ]
        }
      )
    ] })
  ] });
}
function ApprovalChip({ value, disabled, onChange }) {
  const [open, setOpen] = useState2(false);
  const [anchor, setAnchor] = useState2(null);
  const current = APPROVALS.find((item) => item.value === value) ?? APPROVALS[0];
  return /* @__PURE__ */ jsxs(Fragment, { children: [
    /* @__PURE__ */ jsxs(
      "button",
      {
        ref: setAnchor,
        type: "button",
        className: "cc-chip-btn",
        "aria-haspopup": "menu",
        "aria-expanded": open,
        title: `\u5BA1\u6279\u6A21\u5F0F\uFF1A${current.hint}`,
        disabled,
        onClick: () => setOpen((v) => !v),
        children: [
          /* @__PURE__ */ jsx2("span", { className: "cc-chip-icon", children: /* @__PURE__ */ jsx2(ApprovalIcon, { preset: current.value }) }),
          /* @__PURE__ */ jsx2("span", { className: "cc-chip-text", children: current.label }),
          /* @__PURE__ */ jsx2(Chevron, { open })
        ]
      }
    ),
    /* @__PURE__ */ jsx2(Popover, { anchor, open, onClose: () => setOpen(false), width: 220, children: /* @__PURE__ */ jsx2(MenuList, { children: APPROVALS.map((item) => /* @__PURE__ */ jsxs(
      "button",
      {
        type: "button",
        role: "menuitemradio",
        "aria-checked": item.value === value,
        className: "cc-menu-item",
        title: item.hint,
        onClick: () => {
          setOpen(false);
          if (item.value !== value) onChange(item.value);
        },
        children: [
          /* @__PURE__ */ jsx2("span", { className: "cc-menu-item-icon", children: /* @__PURE__ */ jsx2(ApprovalIcon, { preset: item.value }) }),
          /* @__PURE__ */ jsx2("span", { className: "cc-menu-item-text", children: item.label }),
          /* @__PURE__ */ jsx2("span", { className: "cc-menu-check", children: item.value === value && /* @__PURE__ */ jsx2(CheckIcon, {}) })
        ]
      },
      item.value
    )) }) })
  ] });
}
var MODES = [
  { value: "standard", label: "\u6807\u51C6", hint: "\u5E38\u89C4\u5BF9\u8BDD\u4E0E\u4EFB\u52A1\u6267\u884C" },
  { value: "creator", label: "\u521B\u9020", hint: "\u6309\u5951\u7EA6\u5F00\u53D1\u5E76\u5B89\u88C5 ava2 App" }
];
function ModeIcon({ mode }) {
  if (mode === "creator") {
    return /* @__PURE__ */ jsxs("svg", { width: "14", height: "14", viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: "1.8", strokeLinecap: "round", strokeLinejoin: "round", "aria-hidden": "true", children: [
      /* @__PURE__ */ jsx2("path", { d: "M12 3l1.9 5.1L19 10l-5.1 1.9L12 17l-1.9-5.1L5 10l5.1-1.9L12 3Z" }),
      /* @__PURE__ */ jsx2("path", { d: "M19 15.5l.9 2.4 2.4.9-2.4.9-.9 2.4-.9-2.4-2.4-.9 2.4-.9.9-2.4Z" })
    ] });
  }
  return /* @__PURE__ */ jsxs("svg", { width: "14", height: "14", viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: "1.8", strokeLinecap: "round", strokeLinejoin: "round", "aria-hidden": "true", children: [
    /* @__PURE__ */ jsx2("path", { d: "M21 12a9 9 0 1 1-9-9" }),
    /* @__PURE__ */ jsx2("path", { d: "M12 7v5l3 3" })
  ] });
}
function ModeChip({ value, disabled, onChange }) {
  const [open, setOpen] = useState2(false);
  const [anchor, setAnchor] = useState2(null);
  const current = MODES.find((item) => item.value === value) ?? MODES[0];
  return /* @__PURE__ */ jsxs(Fragment, { children: [
    /* @__PURE__ */ jsxs(
      "button",
      {
        ref: setAnchor,
        type: "button",
        className: "cc-chip-btn",
        "aria-haspopup": "menu",
        "aria-expanded": open,
        title: `\u4F1A\u8BDD\u6A21\u5F0F\uFF1A${current.label}\uFF08${current.hint}\uFF09\uFF1B\u9009\u62E9\u540E\u4E0B\u4E00\u6B21\u65B0\u5EFA\u4F1A\u8BDD\u751F\u6548`,
        disabled,
        onClick: () => setOpen((v) => !v),
        children: [
          /* @__PURE__ */ jsx2("span", { className: "cc-chip-icon", children: /* @__PURE__ */ jsx2(ModeIcon, { mode: current.value }) }),
          /* @__PURE__ */ jsx2("span", { className: "cc-chip-text", children: current.label }),
          /* @__PURE__ */ jsx2(Chevron, { open })
        ]
      }
    ),
    /* @__PURE__ */ jsxs(Popover, { anchor, open, onClose: () => setOpen(false), width: 260, children: [
      /* @__PURE__ */ jsx2(MenuList, { children: MODES.map((item) => /* @__PURE__ */ jsxs(
        "button",
        {
          type: "button",
          role: "menuitemradio",
          "aria-checked": item.value === value,
          className: "cc-menu-item",
          title: item.hint,
          onClick: () => {
            setOpen(false);
            if (item.value !== value) onChange(item.value);
          },
          children: [
            /* @__PURE__ */ jsx2("span", { className: "cc-menu-item-icon", children: /* @__PURE__ */ jsx2(ModeIcon, { mode: item.value }) }),
            /* @__PURE__ */ jsx2("span", { className: "cc-menu-item-text", children: item.label }),
            /* @__PURE__ */ jsx2("span", { className: "cc-menu-check", children: item.value === value && /* @__PURE__ */ jsx2(CheckIcon, {}) })
          ]
        },
        item.value
      )) }),
      /* @__PURE__ */ jsx2("div", { className: "cc-menu-hint", children: "\u9009\u62E9\u540E\u4E0B\u4E00\u6B21\u65B0\u5EFA\u4F1A\u8BDD\u751F\u6548" })
    ] })
  ] });
}
function ModelChip({ value, catalog, disabled, onChange }) {
  const [open, setOpen] = useState2(false);
  const [anchor, setAnchor] = useState2(null);
  const [query, setQuery] = useState2("");
  const total = catalog.reduce((n, p) => n + p.models.length, 0);
  const searchable = total > 4;
  const keyword = searchable ? query.trim().toLowerCase() : "";
  const groups = catalog.map((p) => ({
    ...p,
    models: keyword ? p.models.filter((m) => (m.name || m.id).toLowerCase().includes(keyword) || m.id.toLowerCase().includes(keyword)) : p.models
  })).filter((p) => p.models.length > 0);
  const currentName = catalog.flatMap((p) => p.models).find((m) => m.id === value)?.name || value || "\u6A21\u578B\u2026";
  const close = () => {
    setOpen(false);
    setQuery("");
  };
  return /* @__PURE__ */ jsxs(Fragment, { children: [
    /* @__PURE__ */ jsxs(
      "button",
      {
        ref: setAnchor,
        type: "button",
        className: "cc-chip-btn",
        "aria-haspopup": "menu",
        "aria-expanded": open,
        title: `\u6A21\u578B\uFF1A\u5F53\u524D ${currentName}\uFF0C\u5207\u6362\u540E\u4E0B\u4E00\u8BF7\u6C42\u751F\u6548`,
        disabled,
        onClick: () => setOpen((v) => !v),
        children: [
          /* @__PURE__ */ jsx2("span", { className: "cc-chip-icon", children: /* @__PURE__ */ jsx2(ModelIcon, {}) }),
          /* @__PURE__ */ jsx2("span", { className: "cc-chip-text", children: currentName }),
          /* @__PURE__ */ jsx2(Chevron, { open })
        ]
      }
    ),
    /* @__PURE__ */ jsxs(Popover, { anchor, open, onClose: close, width: 300, children: [
      searchable && /* @__PURE__ */ jsx2(
        "input",
        {
          className: "cc-menu-search",
          type: "text",
          role: "searchbox",
          "aria-label": "\u641C\u7D22\u6A21\u578B",
          placeholder: "\u641C\u7D22\u6A21\u578B",
          value: query,
          autoFocus: true,
          onChange: (e) => setQuery(e.target.value)
        }
      ),
      /* @__PURE__ */ jsxs(MenuList, { autoFocusItem: !searchable, children: [
        groups.map((p) => /* @__PURE__ */ jsxs("div", { className: "cc-menu-group", children: [
          /* @__PURE__ */ jsx2("div", { className: "cc-menu-group-head", title: p.name || p.id, children: p.name || p.id }),
          p.models.map((m) => /* @__PURE__ */ jsxs(
            "button",
            {
              type: "button",
              role: "menuitemradio",
              "aria-checked": m.id === value,
              className: "cc-menu-item",
              title: m.description || m.name || m.id,
              onClick: () => {
                close();
                if (m.id !== value) onChange(m.id);
              },
              children: [
                /* @__PURE__ */ jsx2("span", { className: "cc-menu-item-text", children: m.name || m.id }),
                /* @__PURE__ */ jsx2("span", { className: "cc-menu-check", children: m.id === value && /* @__PURE__ */ jsx2(CheckIcon, {}) })
              ]
            },
            m.id
          ))
        ] }, p.id)),
        groups.length === 0 && /* @__PURE__ */ jsx2("div", { className: "cc-menu-empty", children: "\u65E0\u5339\u914D\u6A21\u578B" })
      ] })
    ] })
  ] });
}

// src/frame-reducer.ts
function asRecord(value) {
  return typeof value === "object" && value !== null ? value : void 0;
}
function asString(value) {
  return typeof value === "string" ? value : "";
}
function textBlocks(message) {
  const m = asRecord(message);
  const blocks = Array.isArray(m?.blocks) ? m.blocks : [];
  return blocks.map((b) => asRecord(b)?.type === "text" ? asString(asRecord(b)?.text) : "").join("");
}
function thinkingBlocks(message) {
  const m = asRecord(message);
  const blocks = Array.isArray(m?.blocks) ? m.blocks : [];
  return blocks.map((b) => asRecord(b)?.type === "thinking" ? asString(asRecord(b)?.thinking) : "").filter((text7) => text7 !== "");
}
function imageBlocks(message) {
  const m = asRecord(message);
  const blocks = Array.isArray(m?.blocks) ? m.blocks : [];
  return blocks.flatMap((b) => {
    const block = asRecord(b);
    if (block?.type !== "image") return [];
    const dataUrl = asString(block.image_url);
    if (!dataUrl) return [];
    const mime = /^data:([^;,]+)/.exec(dataUrl)?.[1] ?? "image/*";
    return [{ name: "", mimeType: mime, dataUrl }];
  });
}
function contentText(content3) {
  const blocks = Array.isArray(content3) ? content3 : [];
  return blocks.map((b) => asRecord(b)?.type === "text" ? asString(asRecord(b)?.text) : "").join("");
}
function isInjected(message) {
  const m = asRecord(message);
  return m?.source != null;
}
function asNumber(value) {
  return typeof value === "number" ? value : -1;
}
function asCount(value) {
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}
function turnUsage(raw) {
  const u = asRecord(raw);
  if (!u) return void 0;
  return {
    inputTokens: asCount(u.input_tokens),
    outputTokens: asCount(u.output_tokens),
    cacheReadTokens: asCount(u.cache_read_tokens),
    cacheWriteTokens: asCount(u.cache_write_tokens)
  };
}
function sumUsage(base, add) {
  if (!add) return base;
  const b = base ?? { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheWriteTokens: 0 };
  return {
    inputTokens: b.inputTokens + add.inputTokens,
    outputTokens: b.outputTokens + add.outputTokens,
    cacheReadTokens: b.cacheReadTokens + add.cacheReadTokens,
    cacheWriteTokens: b.cacheWriteTokens + add.cacheWriteTokens
  };
}
function turnReportFromFrame(frame) {
  return {
    turn: asNumber(frame.turn),
    steps: asNumber(frame.steps),
    reason: asString(frame.reason),
    usage: turnUsage(frame.usage)
  };
}
function isTurn(item) {
  return item.kind === "turn";
}
function openTurn(items, turn) {
  const last = items[items.length - 1];
  if (last && isTurn(last) && last.streaming && (last.turn === turn || last.turn === -1)) {
    if (last.turn === -1 && turn !== -1) {
      return [...items.slice(0, -1), { ...last, turn }];
    }
    return items;
  }
  for (let i = items.length - 1; i >= 0; i--) {
    const it = items[i];
    if (isTurn(it) && it.streaming && (it.turn === turn || it.turn === -1)) {
      const patched = it.turn === -1 && turn !== -1 ? { ...it, turn } : it;
      return [...items.slice(0, i), patched, ...items.slice(i + 1)];
    }
    if (isTurn(it) && !it.streaming && it.turn !== -1 && it.turn !== turn) break;
  }
  const fresh = { kind: "turn", turn, thinkingSegs: [], steps: [], answer: "", streaming: true };
  return [...items, fresh];
}
function withLastTurn(items, patch2) {
  const last = items[items.length - 1];
  if (!last || !isTurn(last)) return items;
  return [...items.slice(0, -1), patch2(last)];
}
function mergeAnswer(current, incoming) {
  if (!incoming) return current;
  return current.endsWith(incoming) ? current : current + incoming;
}
function mergeThinkingSegs(turn, blocks) {
  if (blocks.length === 0) return turn.thinkingSegs;
  let segs = turn.thinkingSegs;
  for (const block of blocks) {
    if (segs.length === turn.steps.length + 1) {
      segs = [...segs.slice(0, -1), block];
    } else if (segs[segs.length - 1] !== block) {
      segs = [...segs, block];
    }
  }
  return segs;
}
function withStreamingTurn(items, patch2) {
  for (let i = items.length - 1; i >= 0; i--) {
    const it = items[i];
    if (isTurn(it) && it.streaming) {
      return [...items.slice(0, i), patch2(it), ...items.slice(i + 1)];
    }
  }
  const fresh = { kind: "turn", turn: -1, thinkingSegs: [], steps: [], answer: "", streaming: true };
  return [...items, fresh];
}
function todosFromEvent(raw) {
  const ev = asRecord(raw);
  const list4 = Array.isArray(ev?.todos) ? ev.todos : [];
  return list4.flatMap((t) => {
    const item = asRecord(t);
    const content3 = asString(item?.content).trim();
    const status = item?.status;
    if (!content3) return [];
    return [
      {
        content: content3,
        status: status === "in_progress" ? "in_progress" : status === "completed" ? "completed" : "pending"
      }
    ];
  });
}
function applyDurable(items, raw) {
  const ev = asRecord(raw);
  if (!ev) return items;
  switch (ev.type) {
    case "turn_start":
      return openTurn(items, asNumber(ev.turn));
    case "user_message": {
      if (isInjected(ev.message)) return items;
      const text7 = textBlocks(ev.message);
      const attachments = imageBlocks(ev.message);
      if (!text7 && attachments.length === 0) return items;
      const settled = attachments.length > 0 ? { kind: "user", text: text7, attachments } : { kind: "user", text: text7 };
      for (let i = items.length - 1; i >= 0; i--) {
        const it = items[i];
        if (it.kind === "user" && it.optimistic === true && it.text === text7 && (it.attachments?.length ?? 0) === attachments.length) {
          return items.map((item, j) => j === i ? settled : item);
        }
        if (it.kind === "turn" && !it.streaming && it.turn !== -1) break;
      }
      for (let i = items.length - 1; i >= 0; i--) {
        const it = items[i];
        if (isTurn(it) && it.streaming) {
          return [...items.slice(0, i), settled, ...items.slice(i)];
        }
        if (isTurn(it) && !it.streaming && it.turn !== -1) break;
      }
      return [...items, settled];
    }
    case "tool_call": {
      const call = asRecord(ev.call);
      if (!call) return items;
      const id = asString(call.id);
      const inputText = asString(call.input);
      return withStreamingTurn(openTurn(items, asNumber(ev.turn)), (t) => {
        if (id !== "" && t.steps.some((s) => s.id === id)) {
          return {
            ...t,
            steps: t.steps.map(
              (s) => s.id === id ? { ...s, inputText: inputText || s.inputText } : s
            )
          };
        }
        const step = {
          id,
          name: asString(call.name),
          inputText,
          running: true
        };
        return { ...t, steps: [...t.steps, step] };
      });
    }
    case "tool_result": {
      const result = asRecord(ev.result);
      if (!result) return items;
      const callId = asString(result.tool_call_id);
      const isError = result.is_error === true;
      const resultText = contentText(result.content);
      return withStreamingTurn(items, (t) => ({
        ...t,
        steps: t.steps.map(
          (s) => s.id === callId ? { ...s, running: false, isError, resultText } : s
        )
      }));
    }
    case "assistant_message": {
      const text7 = textBlocks(ev.message);
      const stepUsage = turnUsage(ev.usage);
      return withStreamingTurn(openTurn(items, asNumber(ev.turn)), (t) => ({
        ...t,
        // live 流式累加、durable echo 后缀幂等跳过（回放态逐步累加，中间步
        // 文本不再被末步覆盖——与 live 口径一致）
        answer: mergeAnswer(t.answer, text7),
        usage: sumUsage(t.usage, stepUsage),
        // 落定消息携带本步思考块（onion final_blocks）：见 mergeThinkingSegs
        thinkingSegs: mergeThinkingSegs(t, thinkingBlocks(ev.message))
      }));
    }
    case "assistant_attempt": {
      const error = asString(ev.error);
      if (!error) return items;
      return withStreamingTurn(items, (t) => ({ ...t, retryText: error }));
    }
    case "turn_end": {
      const anyStreaming = items.some((it) => isTurn(it) && it.streaming);
      if (!anyStreaming) return items;
      const reason = asString(ev.reason);
      return items.map((it) => {
        if (!isTurn(it) || !it.streaming) return it;
        return {
          ...it,
          streaming: false,
          report: {
            turn: it.turn,
            steps: it.steps.length,
            reason,
            // turn_report 帧可能先行（其 usage 为权威汇总）；durable echo 补写时保留
            usage: it.usage ?? it.report?.usage
          }
        };
      });
    }
    // ---- 斜杠命令（command_run + command_done 帧） ----
    case "command_run": {
      const id = asString(ev.command_id);
      if (!id) return items;
      const args = asString(ev.args);
      return [
        ...items,
        { kind: "command", id, name: asString(ev.name), args: args ? " " + args : "", outcome: null }
      ];
    }
    case "command_done": {
      const id = asString(ev.command_id);
      if (!id) return items;
      const kind = ev.kind === "error" ? "error" : "success";
      const text7 = asString(ev.text);
      const seq2 = typeof ev.source_event_seq === "number" ? ev.source_event_seq : null;
      return items.map(
        (item) => item.kind === "command" && item.id === id ? { ...item, outcome: { kind, text: text7 }, sourceEventSeq: seq2 } : item
      );
    }
    // ---- 上下文压缩（时间线留一条可展开标记） ----
    case "compaction_start": {
      const last = items[items.length - 1];
      const base = last?.kind === "command" && last.name === "compact" ? items.slice(0, -1) : items;
      return [...base, { kind: "compaction", summary: null, shadowedItemCount: null, shadowedTokenCount: null, running: true }];
    }
    case "compaction_summary": {
      const summary = asString(ev.summary).trim();
      const count = typeof ev.shadowed_count === "number" ? ev.shadowed_count : null;
      const tokens = typeof ev.shadowed_tokens === "number" ? ev.shadowed_tokens : null;
      const marker = {
        kind: "compaction",
        summary: summary === "" ? null : summary,
        shadowedItemCount: count,
        shadowedTokenCount: tokens
      };
      const last = items[items.length - 1];
      if (last?.kind === "compaction" && last.running) {
        return [...items.slice(0, -1), marker];
      }
      return [...items, marker];
    }
    case "compaction_end": {
      const error = asString(ev.error);
      if (!error) return items;
      const last = items[items.length - 1];
      if (last?.kind === "compaction" && last.running) {
        return [...items.slice(0, -1), { ...last, running: false, error }];
      }
      return items;
    }
    default:
      return items;
  }
}
function applyLive(items, raw) {
  const ev = asRecord(raw);
  if (!ev) return items;
  if (ev.Durable !== void 0 || ev.durable !== void 0) return items;
  const subagentEnd = asRecord(ev.SubagentEnd ?? ev.subagent_end);
  if (subagentEnd) {
    const name2 = asString(subagentEnd.name);
    const stop = asString(subagentEnd.stop_reason);
    if (stop && stop !== "completed") {
      return [...items, { kind: "notice", tone: "error", text: `\u5B50\u4EE3\u7406 ${name2} \u7ED3\u675F\uFF08${stop}\uFF09` }];
    }
    return items;
  }
  if (asRecord(ev.SubagentStart ?? ev.subagent_start)) return items;
  const stream = ev.AssistantStream ?? ev.assistant_stream;
  if (stream === void 0 || typeof stream === "string") return items;
  const chunk = asRecord(stream);
  if (!chunk) return items;
  const text7 = typeof chunk.Text === "string" ? chunk.Text : "";
  const thinking = typeof chunk.Thinking === "string" ? chunk.Thinking : "";
  const start2 = asRecord(chunk.ToolCallStart);
  const input2 = asRecord(chunk.ToolCallInput);
  if (text7) {
    return withStreamingTurn(items, (t) => ({ ...t, answer: t.answer + text7 }));
  }
  if (thinking) {
    return withStreamingTurn(items, (t) => {
      const segs = t.thinkingSegs;
      const last = segs.length - 1;
      const next = segs.length === t.steps.length + 1 ? [...segs.slice(0, last), segs[last] + thinking] : [...segs, thinking];
      return { ...t, thinkingSegs: next };
    });
  }
  if (start2) {
    const step = {
      id: asString(start2.id),
      name: asString(start2.name),
      inputText: "",
      running: true
    };
    return withStreamingTurn(items, (t) => ({
      ...t,
      steps: t.steps.some((s) => s.id === step.id && step.id !== "") ? t.steps : [...t.steps, step]
    }));
  }
  if (input2) {
    const index3 = asNumber(input2.index);
    const partial = asString(input2.partial_json);
    return withStreamingTurn(items, (t) => ({
      ...t,
      steps: t.steps.map((s, i) => i === index3 ? { ...s, inputText: s.inputText + partial } : s)
    }));
  }
  return items;
}
function framesToItems(events) {
  const items = events.reduce(applyDurable, []);
  return items.map((it) => {
    if (!isTurn(it) || !it.steps.some((s) => s.running)) return it;
    return {
      ...it,
      steps: it.steps.map(
        (s) => s.running ? { ...s, running: false, isError: true, resultText: s.resultText || "[\u4F1A\u8BDD\u4E2D\u65AD\uFF0C\u65E0\u8FD4\u56DE]" } : s
      )
    };
  });
}
function applyFrame(items, frame) {
  switch (frame.type) {
    case "snapshot": {
      const events = Array.isArray(frame.events) ? frame.events : [];
      return events.reduce(applyDurable, items);
    }
    case "durable":
      return applyDurable(items, frame.event);
    case "live":
      return applyLive(items, frame.event);
    case "turn_report":
      return withLastTurn(items, (t) => ({ ...t, report: turnReportFromFrame(frame) }));
    default:
      return items;
  }
}

// src/format.ts
function truncate(text7, max) {
  return text7.length > max ? text7.slice(0, max) + "\u2026" : text7;
}
function firstLine(text7, max) {
  const line = text7.trim().split("\n").map((s) => s.trim()).find(Boolean) ?? "";
  return truncate(line.replace(/\s+/g, " "), max);
}
function formatTokens(value) {
  if (!Number.isFinite(value) || value <= 0) return "0";
  const scaled = (candidate) => candidate >= 100 ? String(Math.round(candidate)) : String(Math.round(candidate * 10) / 10);
  if (value < 1e3) return String(value);
  if (value < 1e6) return `${scaled(value / 1e3)}K`;
  return `${scaled(value / 1e6)}M`;
}
function formatExactTokens(value) {
  return value.toLocaleString("en-US");
}
function formatCacheHitPercent(cacheReadTokens, promptTokens) {
  if (promptTokens <= 0 || cacheReadTokens <= 0) return null;
  if (cacheReadTokens >= promptTokens) return "100";
  const pct = cacheReadTokens / promptTokens * 100;
  for (let places = 0; places < 6; places++) {
    const text7 = pct.toFixed(places);
    if (Number(text7) < 100) return text7.replace(/\.?0+$/, "");
  }
  return "99.999999";
}
function usageTotal(usage) {
  if (!usage) return 0;
  return usage.inputTokens + usage.outputTokens;
}
function turnUsageOf(item) {
  return item.report?.usage ?? item.usage;
}

// src/items.tsx
import { Fragment as Fragment3, useState as useState4 } from "react";

// node_modules/devlop/lib/default.js
function ok() {
}
function unreachable() {
}

// node_modules/comma-separated-tokens/index.js
function stringify(values, options) {
  const settings = options || {};
  const input2 = values[values.length - 1] === "" ? [...values, ""] : values;
  return input2.join(
    (settings.padRight ? " " : "") + "," + (settings.padLeft === false ? "" : " ")
  ).trim();
}

// node_modules/estree-util-is-identifier-name/lib/index.js
var nameRe = /^[$_\p{ID_Start}][$_\u{200C}\u{200D}\p{ID_Continue}]*$/u;
var nameReJsx = /^[$_\p{ID_Start}][-$_\u{200C}\u{200D}\p{ID_Continue}]*$/u;
var emptyOptions = {};
function name(name2, options) {
  const settings = options || emptyOptions;
  const re2 = settings.jsx ? nameReJsx : nameRe;
  return re2.test(name2);
}

// node_modules/hast-util-whitespace/lib/index.js
var re = /[ \t\n\f\r]/g;
function whitespace(thing) {
  return typeof thing === "object" ? thing.type === "text" ? empty(thing.value) : false : empty(thing);
}
function empty(value) {
  return value.replace(re, "") === "";
}

// node_modules/property-information/lib/util/schema.js
var Schema = class {
  /**
   * @param {SchemaType['property']} property
   *   Property.
   * @param {SchemaType['normal']} normal
   *   Normal.
   * @param {Space | undefined} [space]
   *   Space.
   * @returns
   *   Schema.
   */
  constructor(property, normal, space2) {
    this.normal = normal;
    this.property = property;
    if (space2) {
      this.space = space2;
    }
  }
};
Schema.prototype.normal = {};
Schema.prototype.property = {};
Schema.prototype.space = void 0;

// node_modules/property-information/lib/util/merge.js
function merge(definitions, space2) {
  const property = {};
  const normal = {};
  for (const definition3 of definitions) {
    Object.assign(property, definition3.property);
    Object.assign(normal, definition3.normal);
  }
  return new Schema(property, normal, space2);
}

// node_modules/property-information/lib/normalize.js
function normalize(value) {
  return value.toLowerCase();
}

// node_modules/property-information/lib/util/info.js
var Info = class {
  /**
   * @param {string} property
   *   Property.
   * @param {string} attribute
   *   Attribute.
   * @returns
   *   Info.
   */
  constructor(property, attribute) {
    this.attribute = attribute;
    this.property = property;
  }
};
Info.prototype.attribute = "";
Info.prototype.booleanish = false;
Info.prototype.boolean = false;
Info.prototype.commaOrSpaceSeparated = false;
Info.prototype.commaSeparated = false;
Info.prototype.defined = false;
Info.prototype.mustUseProperty = false;
Info.prototype.number = false;
Info.prototype.overloadedBoolean = false;
Info.prototype.property = "";
Info.prototype.spaceSeparated = false;
Info.prototype.space = void 0;

// node_modules/property-information/lib/util/types.js
var types_exports = {};
__export(types_exports, {
  boolean: () => boolean,
  booleanish: () => booleanish,
  commaOrSpaceSeparated: () => commaOrSpaceSeparated,
  commaSeparated: () => commaSeparated,
  number: () => number,
  overloadedBoolean: () => overloadedBoolean,
  spaceSeparated: () => spaceSeparated
});
var powers = 0;
var boolean = increment();
var booleanish = increment();
var overloadedBoolean = increment();
var number = increment();
var spaceSeparated = increment();
var commaSeparated = increment();
var commaOrSpaceSeparated = increment();
function increment() {
  return 2 ** ++powers;
}

// node_modules/property-information/lib/util/defined-info.js
var checks = (
  /** @type {ReadonlyArray<keyof typeof types>} */
  Object.keys(types_exports)
);
var DefinedInfo = class extends Info {
  /**
   * @constructor
   * @param {string} property
   *   Property.
   * @param {string} attribute
   *   Attribute.
   * @param {number | null | undefined} [mask]
   *   Mask.
   * @param {Space | undefined} [space]
   *   Space.
   * @returns
   *   Info.
   */
  constructor(property, attribute, mask, space2) {
    let index3 = -1;
    super(property, attribute);
    mark(this, "space", space2);
    if (typeof mask === "number") {
      while (++index3 < checks.length) {
        const check2 = checks[index3];
        mark(this, checks[index3], (mask & types_exports[check2]) === types_exports[check2]);
      }
    }
  }
};
DefinedInfo.prototype.defined = true;
function mark(values, key, value) {
  if (value) {
    values[key] = value;
  }
}

// node_modules/property-information/lib/util/create.js
function create(definition3) {
  const properties = {};
  const normals = {};
  for (const [property, value] of Object.entries(definition3.properties)) {
    const info = new DefinedInfo(
      property,
      definition3.transform(definition3.attributes || {}, property),
      value,
      definition3.space
    );
    if (definition3.mustUseProperty && definition3.mustUseProperty.includes(property)) {
      info.mustUseProperty = true;
    }
    properties[property] = info;
    normals[normalize(property)] = property;
    normals[normalize(info.attribute)] = property;
  }
  return new Schema(properties, normals, definition3.space);
}

// node_modules/property-information/lib/aria.js
var aria = create({
  properties: {
    ariaActiveDescendant: null,
    ariaAtomic: booleanish,
    ariaAutoComplete: null,
    ariaBusy: booleanish,
    ariaChecked: booleanish,
    ariaColCount: number,
    ariaColIndex: number,
    ariaColSpan: number,
    ariaControls: spaceSeparated,
    ariaCurrent: null,
    ariaDescribedBy: spaceSeparated,
    ariaDetails: null,
    ariaDisabled: booleanish,
    ariaDropEffect: spaceSeparated,
    ariaErrorMessage: null,
    ariaExpanded: booleanish,
    ariaFlowTo: spaceSeparated,
    ariaGrabbed: booleanish,
    ariaHasPopup: null,
    ariaHidden: booleanish,
    ariaInvalid: null,
    ariaKeyShortcuts: null,
    ariaLabel: null,
    ariaLabelledBy: spaceSeparated,
    ariaLevel: number,
    ariaLive: null,
    ariaModal: booleanish,
    ariaMultiLine: booleanish,
    ariaMultiSelectable: booleanish,
    ariaOrientation: null,
    ariaOwns: spaceSeparated,
    ariaPlaceholder: null,
    ariaPosInSet: number,
    ariaPressed: booleanish,
    ariaReadOnly: booleanish,
    ariaRelevant: null,
    ariaRequired: booleanish,
    ariaRoleDescription: spaceSeparated,
    ariaRowCount: number,
    ariaRowIndex: number,
    ariaRowSpan: number,
    ariaSelected: booleanish,
    ariaSetSize: number,
    ariaSort: null,
    ariaValueMax: number,
    ariaValueMin: number,
    ariaValueNow: number,
    ariaValueText: null,
    role: null
  },
  transform(_, property) {
    return property === "role" ? property : "aria-" + property.slice(4).toLowerCase();
  }
});

// node_modules/property-information/lib/util/case-sensitive-transform.js
function caseSensitiveTransform(attributes, attribute) {
  return attribute in attributes ? attributes[attribute] : attribute;
}

// node_modules/property-information/lib/util/case-insensitive-transform.js
function caseInsensitiveTransform(attributes, property) {
  return caseSensitiveTransform(attributes, property.toLowerCase());
}

// node_modules/property-information/lib/html.js
var html = create({
  attributes: {
    acceptcharset: "accept-charset",
    classname: "class",
    htmlfor: "for",
    httpequiv: "http-equiv"
  },
  mustUseProperty: ["checked", "multiple", "muted", "selected"],
  properties: {
    // Standard Properties.
    abbr: null,
    accept: commaSeparated,
    acceptCharset: spaceSeparated,
    accessKey: spaceSeparated,
    action: null,
    allow: null,
    allowFullScreen: boolean,
    allowPaymentRequest: boolean,
    allowUserMedia: boolean,
    alpha: boolean,
    alt: null,
    as: null,
    async: boolean,
    autoCapitalize: null,
    autoComplete: spaceSeparated,
    autoFocus: boolean,
    autoPlay: boolean,
    blocking: spaceSeparated,
    capture: null,
    charSet: null,
    checked: boolean,
    cite: null,
    className: spaceSeparated,
    closedBy: null,
    colorSpace: null,
    cols: number,
    colSpan: number,
    command: null,
    commandFor: null,
    content: null,
    contentEditable: booleanish,
    controls: boolean,
    controlsList: spaceSeparated,
    coords: number | commaSeparated,
    crossOrigin: null,
    data: null,
    dateTime: null,
    decoding: null,
    default: boolean,
    defer: boolean,
    dir: null,
    dirName: null,
    disabled: boolean,
    download: overloadedBoolean,
    draggable: booleanish,
    encType: null,
    enterKeyHint: null,
    fetchPriority: null,
    form: null,
    formAction: null,
    formEncType: null,
    formMethod: null,
    formNoValidate: boolean,
    formTarget: null,
    headers: spaceSeparated,
    height: number,
    hidden: overloadedBoolean,
    high: number,
    href: null,
    hrefLang: null,
    htmlFor: spaceSeparated,
    httpEquiv: spaceSeparated,
    id: null,
    imageSizes: null,
    imageSrcSet: null,
    inert: boolean,
    inputMode: null,
    integrity: null,
    is: null,
    isMap: boolean,
    itemId: null,
    itemProp: spaceSeparated,
    itemRef: spaceSeparated,
    itemScope: boolean,
    itemType: spaceSeparated,
    kind: null,
    label: null,
    lang: null,
    language: null,
    list: null,
    loading: null,
    loop: boolean,
    low: number,
    manifest: null,
    max: null,
    maxLength: number,
    media: null,
    method: null,
    min: null,
    minLength: number,
    multiple: boolean,
    muted: boolean,
    name: null,
    nonce: null,
    noModule: boolean,
    noValidate: boolean,
    onAbort: null,
    onAfterPrint: null,
    onAuxClick: null,
    onBeforeMatch: null,
    onBeforePrint: null,
    onBeforeToggle: null,
    onBeforeUnload: null,
    onBlur: null,
    onCancel: null,
    onCanPlay: null,
    onCanPlayThrough: null,
    onChange: null,
    onClick: null,
    onClose: null,
    onContextLost: null,
    onContextMenu: null,
    onContextRestored: null,
    onCopy: null,
    onCueChange: null,
    onCut: null,
    onDblClick: null,
    onDrag: null,
    onDragEnd: null,
    onDragEnter: null,
    onDragExit: null,
    onDragLeave: null,
    onDragOver: null,
    onDragStart: null,
    onDrop: null,
    onDurationChange: null,
    onEmptied: null,
    onEnded: null,
    onError: null,
    onFocus: null,
    onFormData: null,
    onHashChange: null,
    onInput: null,
    onInvalid: null,
    onKeyDown: null,
    onKeyPress: null,
    onKeyUp: null,
    onLanguageChange: null,
    onLoad: null,
    onLoadedData: null,
    onLoadedMetadata: null,
    onLoadEnd: null,
    onLoadStart: null,
    onMessage: null,
    onMessageError: null,
    onMouseDown: null,
    onMouseEnter: null,
    onMouseLeave: null,
    onMouseMove: null,
    onMouseOut: null,
    onMouseOver: null,
    onMouseUp: null,
    onOffline: null,
    onOnline: null,
    onPageHide: null,
    onPageShow: null,
    onPaste: null,
    onPause: null,
    onPlay: null,
    onPlaying: null,
    onPopState: null,
    onProgress: null,
    onRateChange: null,
    onRejectionHandled: null,
    onReset: null,
    onResize: null,
    onScroll: null,
    onScrollEnd: null,
    onSecurityPolicyViolation: null,
    onSeeked: null,
    onSeeking: null,
    onSelect: null,
    onSlotChange: null,
    onStalled: null,
    onStorage: null,
    onSubmit: null,
    onSuspend: null,
    onTimeUpdate: null,
    onToggle: null,
    onUnhandledRejection: null,
    onUnload: null,
    onVolumeChange: null,
    onWaiting: null,
    onWheel: null,
    open: boolean,
    optimum: number,
    pattern: null,
    ping: spaceSeparated,
    placeholder: null,
    playsInline: boolean,
    popover: null,
    popoverTarget: null,
    popoverTargetAction: null,
    poster: null,
    preload: null,
    readOnly: boolean,
    referrerPolicy: null,
    rel: spaceSeparated,
    required: boolean,
    reversed: boolean,
    rows: number,
    rowSpan: number,
    sandbox: spaceSeparated,
    scope: null,
    scoped: boolean,
    seamless: boolean,
    selected: boolean,
    shadowRootClonable: boolean,
    shadowRootCustomElementRegistry: boolean,
    shadowRootDelegatesFocus: boolean,
    shadowRootMode: null,
    shadowRootSerializable: boolean,
    shape: null,
    size: number,
    sizes: null,
    slot: null,
    span: number,
    spellCheck: booleanish,
    src: null,
    srcDoc: null,
    srcLang: null,
    srcSet: null,
    start: number,
    step: null,
    style: null,
    tabIndex: number,
    target: null,
    title: null,
    translate: null,
    type: null,
    typeMustMatch: boolean,
    useMap: null,
    value: booleanish,
    width: number,
    wrap: null,
    writingSuggestions: null,
    // Legacy.
    // See: https://html.spec.whatwg.org/#other-elements,-attributes-and-apis
    align: null,
    // Several. Use CSS `text-align` instead,
    aLink: null,
    // `<body>`. Use CSS `a:active {color}` instead
    archive: spaceSeparated,
    // `<object>`. List of URIs to archives
    axis: null,
    // `<td>` and `<th>`. Use `scope` on `<th>`
    background: null,
    // `<body>`. Use CSS `background-image` instead
    bgColor: null,
    // `<body>` and table elements. Use CSS `background-color` instead
    border: number,
    // `<table>`. Use CSS `border-width` instead,
    borderColor: null,
    // `<table>`. Use CSS `border-color` instead,
    bottomMargin: number,
    // `<body>`
    cellPadding: null,
    // `<table>`
    cellSpacing: null,
    // `<table>`
    char: null,
    // Several table elements. When `align=char`, sets the character to align on
    charOff: null,
    // Several table elements. When `char`, offsets the alignment
    classId: null,
    // `<object>`
    clear: null,
    // `<br>`. Use CSS `clear` instead
    code: null,
    // `<object>`
    codeBase: null,
    // `<object>`
    codeType: null,
    // `<object>`
    color: null,
    // `<font>` and `<hr>`. Use CSS instead
    compact: boolean,
    // Lists. Use CSS to reduce space between items instead
    declare: boolean,
    // `<object>`
    event: null,
    // `<script>`
    face: null,
    // `<font>`. Use CSS instead
    frame: null,
    // `<table>`
    frameBorder: null,
    // `<iframe>`. Use CSS `border` instead
    hSpace: number,
    // `<img>` and `<object>`
    leftMargin: number,
    // `<body>`
    link: null,
    // `<body>`. Use CSS `a:link {color: *}` instead
    longDesc: null,
    // `<frame>`, `<iframe>`, and `<img>`. Use an `<a>`
    lowSrc: null,
    // `<img>`. Use a `<picture>`
    marginHeight: number,
    // `<body>`
    marginWidth: number,
    // `<body>`
    noResize: boolean,
    // `<frame>`
    noHref: boolean,
    // `<area>`. Use no href instead of an explicit `nohref`
    noShade: boolean,
    // `<hr>`. Use background-color and height instead of borders
    noWrap: boolean,
    // `<td>` and `<th>`
    object: null,
    // `<applet>`
    profile: null,
    // `<head>`
    prompt: null,
    // `<isindex>`
    rev: null,
    // `<link>`
    rightMargin: number,
    // `<body>`
    rules: null,
    // `<table>`
    scheme: null,
    // `<meta>`
    scrolling: booleanish,
    // `<frame>`. Use overflow in the child context
    standby: null,
    // `<object>`
    summary: null,
    // `<table>`
    text: null,
    // `<body>`. Use CSS `color` instead
    topMargin: number,
    // `<body>`
    valueType: null,
    // `<param>`
    version: null,
    // `<html>`. Use a doctype.
    vAlign: null,
    // Several. Use CSS `vertical-align` instead
    vLink: null,
    // `<body>`. Use CSS `a:visited {color}` instead
    vSpace: number,
    // `<img>` and `<object>`
    // Non-standard Properties.
    allowTransparency: null,
    autoCorrect: null,
    autoSave: null,
    credentialless: boolean,
    disablePictureInPicture: boolean,
    disableRemotePlayback: boolean,
    exportParts: commaSeparated,
    part: spaceSeparated,
    prefix: null,
    property: null,
    results: number,
    security: null,
    unselectable: null
  },
  space: "html",
  transform: caseInsensitiveTransform
});

// node_modules/property-information/lib/svg.js
var svg = create({
  attributes: {
    accentHeight: "accent-height",
    alignmentBaseline: "alignment-baseline",
    arabicForm: "arabic-form",
    baselineShift: "baseline-shift",
    capHeight: "cap-height",
    className: "class",
    clipPath: "clip-path",
    clipRule: "clip-rule",
    colorInterpolation: "color-interpolation",
    colorInterpolationFilters: "color-interpolation-filters",
    colorProfile: "color-profile",
    colorRendering: "color-rendering",
    crossOrigin: "crossorigin",
    dataType: "datatype",
    dominantBaseline: "dominant-baseline",
    enableBackground: "enable-background",
    fillOpacity: "fill-opacity",
    fillRule: "fill-rule",
    floodColor: "flood-color",
    floodOpacity: "flood-opacity",
    fontFamily: "font-family",
    fontSize: "font-size",
    fontSizeAdjust: "font-size-adjust",
    fontStretch: "font-stretch",
    fontStyle: "font-style",
    fontVariant: "font-variant",
    fontWeight: "font-weight",
    glyphName: "glyph-name",
    glyphOrientationHorizontal: "glyph-orientation-horizontal",
    glyphOrientationVertical: "glyph-orientation-vertical",
    hrefLang: "hreflang",
    horizAdvX: "horiz-adv-x",
    horizOriginX: "horiz-origin-x",
    horizOriginY: "horiz-origin-y",
    imageRendering: "image-rendering",
    letterSpacing: "letter-spacing",
    lightingColor: "lighting-color",
    markerEnd: "marker-end",
    markerMid: "marker-mid",
    markerStart: "marker-start",
    maskType: "mask-type",
    navDown: "nav-down",
    navDownLeft: "nav-down-left",
    navDownRight: "nav-down-right",
    navLeft: "nav-left",
    navNext: "nav-next",
    navPrev: "nav-prev",
    navRight: "nav-right",
    navUp: "nav-up",
    navUpLeft: "nav-up-left",
    navUpRight: "nav-up-right",
    onAbort: "onabort",
    onActivate: "onactivate",
    onAfterPrint: "onafterprint",
    onBeforePrint: "onbeforeprint",
    onBegin: "onbegin",
    onCancel: "oncancel",
    onCanPlay: "oncanplay",
    onCanPlayThrough: "oncanplaythrough",
    onChange: "onchange",
    onClick: "onclick",
    onClose: "onclose",
    onCopy: "oncopy",
    onCueChange: "oncuechange",
    onCut: "oncut",
    onDblClick: "ondblclick",
    onDrag: "ondrag",
    onDragEnd: "ondragend",
    onDragEnter: "ondragenter",
    onDragExit: "ondragexit",
    onDragLeave: "ondragleave",
    onDragOver: "ondragover",
    onDragStart: "ondragstart",
    onDrop: "ondrop",
    onDurationChange: "ondurationchange",
    onEmptied: "onemptied",
    onEnd: "onend",
    onEnded: "onended",
    onError: "onerror",
    onFocus: "onfocus",
    onFocusIn: "onfocusin",
    onFocusOut: "onfocusout",
    onHashChange: "onhashchange",
    onInput: "oninput",
    onInvalid: "oninvalid",
    onKeyDown: "onkeydown",
    onKeyPress: "onkeypress",
    onKeyUp: "onkeyup",
    onLoad: "onload",
    onLoadedData: "onloadeddata",
    onLoadedMetadata: "onloadedmetadata",
    onLoadStart: "onloadstart",
    onMessage: "onmessage",
    onMouseDown: "onmousedown",
    onMouseEnter: "onmouseenter",
    onMouseLeave: "onmouseleave",
    onMouseMove: "onmousemove",
    onMouseOut: "onmouseout",
    onMouseOver: "onmouseover",
    onMouseUp: "onmouseup",
    onMouseWheel: "onmousewheel",
    onOffline: "onoffline",
    onOnline: "ononline",
    onPageHide: "onpagehide",
    onPageShow: "onpageshow",
    onPaste: "onpaste",
    onPause: "onpause",
    onPlay: "onplay",
    onPlaying: "onplaying",
    onPopState: "onpopstate",
    onProgress: "onprogress",
    onRateChange: "onratechange",
    onRepeat: "onrepeat",
    onReset: "onreset",
    onResize: "onresize",
    onScroll: "onscroll",
    onSeeked: "onseeked",
    onSeeking: "onseeking",
    onSelect: "onselect",
    onShow: "onshow",
    onStalled: "onstalled",
    onStorage: "onstorage",
    onSubmit: "onsubmit",
    onSuspend: "onsuspend",
    onTimeUpdate: "ontimeupdate",
    onToggle: "ontoggle",
    onUnload: "onunload",
    onVolumeChange: "onvolumechange",
    onWaiting: "onwaiting",
    onZoom: "onzoom",
    overlinePosition: "overline-position",
    overlineThickness: "overline-thickness",
    paintOrder: "paint-order",
    panose1: "panose-1",
    pointerEvents: "pointer-events",
    referrerPolicy: "referrerpolicy",
    renderingIntent: "rendering-intent",
    shapeRendering: "shape-rendering",
    stopColor: "stop-color",
    stopOpacity: "stop-opacity",
    strikethroughPosition: "strikethrough-position",
    strikethroughThickness: "strikethrough-thickness",
    strokeDashArray: "stroke-dasharray",
    strokeDashOffset: "stroke-dashoffset",
    strokeLineCap: "stroke-linecap",
    strokeLineJoin: "stroke-linejoin",
    strokeMiterLimit: "stroke-miterlimit",
    strokeOpacity: "stroke-opacity",
    strokeWidth: "stroke-width",
    tabIndex: "tabindex",
    textAnchor: "text-anchor",
    textDecoration: "text-decoration",
    textRendering: "text-rendering",
    transformOrigin: "transform-origin",
    typeOf: "typeof",
    underlinePosition: "underline-position",
    underlineThickness: "underline-thickness",
    unicodeBidi: "unicode-bidi",
    unicodeRange: "unicode-range",
    unitsPerEm: "units-per-em",
    vAlphabetic: "v-alphabetic",
    vHanging: "v-hanging",
    vIdeographic: "v-ideographic",
    vMathematical: "v-mathematical",
    vectorEffect: "vector-effect",
    vertAdvY: "vert-adv-y",
    vertOriginX: "vert-origin-x",
    vertOriginY: "vert-origin-y",
    wordSpacing: "word-spacing",
    writingMode: "writing-mode",
    xHeight: "x-height",
    // These were camelcased in Tiny. Now lowercased in SVG 2
    playbackOrder: "playbackorder",
    timelineBegin: "timelinebegin"
  },
  properties: {
    about: commaOrSpaceSeparated,
    accentHeight: number,
    accumulate: null,
    additive: null,
    alignmentBaseline: null,
    alphabetic: number,
    amplitude: number,
    arabicForm: null,
    ascent: number,
    attributeName: null,
    attributeType: null,
    azimuth: number,
    bandwidth: null,
    baselineShift: null,
    baseFrequency: null,
    baseProfile: null,
    bbox: null,
    begin: null,
    bias: number,
    by: null,
    calcMode: null,
    capHeight: number,
    className: spaceSeparated,
    clip: null,
    clipPath: null,
    clipPathUnits: null,
    clipRule: null,
    color: null,
    colorInterpolation: null,
    colorInterpolationFilters: null,
    colorProfile: null,
    colorRendering: null,
    content: null,
    contentScriptType: null,
    contentStyleType: null,
    crossOrigin: null,
    cursor: null,
    cx: null,
    cy: null,
    d: null,
    dataType: null,
    defaultAction: null,
    descent: number,
    diffuseConstant: number,
    direction: null,
    display: null,
    dur: null,
    divisor: number,
    dominantBaseline: null,
    download: boolean,
    dx: null,
    dy: null,
    edgeMode: null,
    editable: null,
    elevation: number,
    enableBackground: null,
    end: null,
    event: null,
    exponent: number,
    externalResourcesRequired: null,
    fill: null,
    fillOpacity: number,
    fillRule: null,
    filter: null,
    filterRes: null,
    filterUnits: null,
    floodColor: null,
    floodOpacity: null,
    focusable: null,
    focusHighlight: null,
    fontFamily: null,
    fontSize: null,
    fontSizeAdjust: null,
    fontStretch: null,
    fontStyle: null,
    fontVariant: null,
    fontWeight: null,
    format: null,
    fr: null,
    from: null,
    fx: null,
    fy: null,
    g1: commaSeparated,
    g2: commaSeparated,
    glyphName: commaSeparated,
    glyphOrientationHorizontal: null,
    glyphOrientationVertical: null,
    glyphRef: null,
    gradientTransform: null,
    gradientUnits: null,
    handler: null,
    hanging: number,
    hatchContentUnits: null,
    hatchUnits: null,
    height: null,
    href: null,
    hrefLang: null,
    horizAdvX: number,
    horizOriginX: number,
    horizOriginY: number,
    id: null,
    ideographic: number,
    imageRendering: null,
    initialVisibility: null,
    in: null,
    in2: null,
    intercept: number,
    k: number,
    k1: number,
    k2: number,
    k3: number,
    k4: number,
    kernelMatrix: commaOrSpaceSeparated,
    kernelUnitLength: null,
    keyPoints: null,
    // SEMI_COLON_SEPARATED
    keySplines: null,
    // SEMI_COLON_SEPARATED
    keyTimes: null,
    // SEMI_COLON_SEPARATED
    kerning: null,
    lang: null,
    lengthAdjust: null,
    letterSpacing: null,
    lightingColor: null,
    limitingConeAngle: number,
    local: null,
    markerEnd: null,
    markerMid: null,
    markerStart: null,
    markerHeight: null,
    markerUnits: null,
    markerWidth: null,
    mask: null,
    maskContentUnits: null,
    maskType: null,
    maskUnits: null,
    mathematical: null,
    max: null,
    media: null,
    mediaCharacterEncoding: null,
    mediaContentEncodings: null,
    mediaSize: number,
    mediaTime: null,
    method: null,
    min: null,
    mode: null,
    name: null,
    navDown: null,
    navDownLeft: null,
    navDownRight: null,
    navLeft: null,
    navNext: null,
    navPrev: null,
    navRight: null,
    navUp: null,
    navUpLeft: null,
    navUpRight: null,
    numOctaves: null,
    observer: null,
    offset: null,
    onAbort: null,
    onActivate: null,
    onAfterPrint: null,
    onBeforePrint: null,
    onBegin: null,
    onCancel: null,
    onCanPlay: null,
    onCanPlayThrough: null,
    onChange: null,
    onClick: null,
    onClose: null,
    onCopy: null,
    onCueChange: null,
    onCut: null,
    onDblClick: null,
    onDrag: null,
    onDragEnd: null,
    onDragEnter: null,
    onDragExit: null,
    onDragLeave: null,
    onDragOver: null,
    onDragStart: null,
    onDrop: null,
    onDurationChange: null,
    onEmptied: null,
    onEnd: null,
    onEnded: null,
    onError: null,
    onFocus: null,
    onFocusIn: null,
    onFocusOut: null,
    onHashChange: null,
    onInput: null,
    onInvalid: null,
    onKeyDown: null,
    onKeyPress: null,
    onKeyUp: null,
    onLoad: null,
    onLoadedData: null,
    onLoadedMetadata: null,
    onLoadStart: null,
    onMessage: null,
    onMouseDown: null,
    onMouseEnter: null,
    onMouseLeave: null,
    onMouseMove: null,
    onMouseOut: null,
    onMouseOver: null,
    onMouseUp: null,
    onMouseWheel: null,
    onOffline: null,
    onOnline: null,
    onPageHide: null,
    onPageShow: null,
    onPaste: null,
    onPause: null,
    onPlay: null,
    onPlaying: null,
    onPopState: null,
    onProgress: null,
    onRateChange: null,
    onRepeat: null,
    onReset: null,
    onResize: null,
    onScroll: null,
    onSeeked: null,
    onSeeking: null,
    onSelect: null,
    onShow: null,
    onStalled: null,
    onStorage: null,
    onSubmit: null,
    onSuspend: null,
    onTimeUpdate: null,
    onToggle: null,
    onUnload: null,
    onVolumeChange: null,
    onWaiting: null,
    onZoom: null,
    opacity: null,
    operator: null,
    order: null,
    orient: null,
    orientation: null,
    origin: null,
    overflow: null,
    overlay: null,
    overlinePosition: number,
    overlineThickness: number,
    paintOrder: null,
    panose1: null,
    path: null,
    pathLength: number,
    patternContentUnits: null,
    patternTransform: null,
    patternUnits: null,
    phase: null,
    ping: spaceSeparated,
    pitch: null,
    playbackOrder: null,
    pointerEvents: null,
    points: null,
    pointsAtX: number,
    pointsAtY: number,
    pointsAtZ: number,
    preserveAlpha: null,
    preserveAspectRatio: null,
    primitiveUnits: null,
    propagate: null,
    property: commaOrSpaceSeparated,
    r: null,
    radius: null,
    referrerPolicy: null,
    refX: null,
    refY: null,
    rel: commaOrSpaceSeparated,
    rev: commaOrSpaceSeparated,
    renderingIntent: null,
    repeatCount: null,
    repeatDur: null,
    requiredExtensions: commaOrSpaceSeparated,
    requiredFeatures: commaOrSpaceSeparated,
    requiredFonts: commaOrSpaceSeparated,
    requiredFormats: commaOrSpaceSeparated,
    resource: null,
    restart: null,
    result: null,
    rotate: null,
    rx: null,
    ry: null,
    scale: null,
    seed: null,
    shapeRendering: null,
    side: null,
    slope: null,
    snapshotTime: null,
    specularConstant: number,
    specularExponent: number,
    spreadMethod: null,
    spacing: null,
    startOffset: null,
    stdDeviation: null,
    stemh: null,
    stemv: null,
    stitchTiles: null,
    stopColor: null,
    stopOpacity: null,
    strikethroughPosition: number,
    strikethroughThickness: number,
    string: null,
    stroke: null,
    strokeDashArray: commaOrSpaceSeparated,
    strokeDashOffset: null,
    strokeLineCap: null,
    strokeLineJoin: null,
    strokeMiterLimit: number,
    strokeOpacity: number,
    strokeWidth: null,
    style: null,
    surfaceScale: number,
    syncBehavior: null,
    syncBehaviorDefault: null,
    syncMaster: null,
    syncTolerance: null,
    syncToleranceDefault: null,
    systemLanguage: commaOrSpaceSeparated,
    tabIndex: number,
    tableValues: null,
    target: null,
    targetX: number,
    targetY: number,
    textAnchor: null,
    textDecoration: null,
    textRendering: null,
    textLength: null,
    timelineBegin: null,
    title: null,
    transformBehavior: null,
    type: null,
    typeOf: commaOrSpaceSeparated,
    to: null,
    transform: null,
    transformOrigin: null,
    u1: null,
    u2: null,
    underlinePosition: number,
    underlineThickness: number,
    unicode: null,
    unicodeBidi: null,
    unicodeRange: null,
    unitsPerEm: number,
    values: null,
    vAlphabetic: number,
    vMathematical: number,
    vectorEffect: null,
    vHanging: number,
    vIdeographic: number,
    version: null,
    vertAdvY: number,
    vertOriginX: number,
    vertOriginY: number,
    viewBox: null,
    viewTarget: null,
    visibility: null,
    width: null,
    widths: null,
    wordSpacing: null,
    writingMode: null,
    x: null,
    x1: null,
    x2: null,
    xChannelSelector: null,
    xHeight: number,
    y: null,
    y1: null,
    y2: null,
    yChannelSelector: null,
    z: null,
    zoomAndPan: null
  },
  space: "svg",
  transform: caseSensitiveTransform
});

// node_modules/property-information/lib/xlink.js
var xlink = create({
  properties: {
    xLinkActuate: null,
    xLinkArcRole: null,
    xLinkHref: null,
    xLinkRole: null,
    xLinkShow: null,
    xLinkTitle: null,
    xLinkType: null
  },
  space: "xlink",
  transform(_, property) {
    return "xlink:" + property.slice(5).toLowerCase();
  }
});

// node_modules/property-information/lib/xmlns.js
var xmlns = create({
  attributes: { xmlnsxlink: "xmlns:xlink" },
  properties: { xmlnsXLink: null, xmlns: null },
  space: "xmlns",
  transform: caseInsensitiveTransform
});

// node_modules/property-information/lib/xml.js
var xml = create({
  properties: { xmlBase: null, xmlLang: null, xmlSpace: null },
  space: "xml",
  transform(_, property) {
    return "xml:" + property.slice(3).toLowerCase();
  }
});

// node_modules/property-information/lib/hast-to-react.js
var hastToReact = {
  classId: "classID",
  dataType: "datatype",
  itemId: "itemID",
  strokeDashArray: "strokeDasharray",
  strokeDashOffset: "strokeDashoffset",
  strokeLineCap: "strokeLinecap",
  strokeLineJoin: "strokeLinejoin",
  strokeMiterLimit: "strokeMiterlimit",
  typeOf: "typeof",
  xLinkActuate: "xlinkActuate",
  xLinkArcRole: "xlinkArcrole",
  xLinkHref: "xlinkHref",
  xLinkRole: "xlinkRole",
  xLinkShow: "xlinkShow",
  xLinkTitle: "xlinkTitle",
  xLinkType: "xlinkType",
  xmlnsXLink: "xmlnsXlink"
};

// node_modules/property-information/lib/find.js
var cap = /[A-Z]/g;
var dash = /-[a-z]/g;
var valid = /^data[-\w.:]+$/i;
function find(schema, value) {
  const normal = normalize(value);
  let property = value;
  let Type = Info;
  if (normal in schema.normal) {
    return schema.property[schema.normal[normal]];
  }
  if (normal.length > 4 && normal.slice(0, 4) === "data" && valid.test(value)) {
    if (value.charAt(4) === "-") {
      const rest = value.slice(5).replace(dash, camelcase);
      property = "data" + rest.charAt(0).toUpperCase() + rest.slice(1);
    } else {
      const rest = value.slice(4);
      if (!dash.test(rest)) {
        let dashes = rest.replace(cap, kebab);
        if (dashes.charAt(0) !== "-") {
          dashes = "-" + dashes;
        }
        value = "data" + dashes;
      }
    }
    Type = DefinedInfo;
  }
  return new Type(property, value);
}
function kebab($0) {
  return "-" + $0.toLowerCase();
}
function camelcase($0) {
  return $0.charAt(1).toUpperCase();
}

// node_modules/property-information/index.js
var html2 = merge([aria, html, xlink, xmlns, xml], "html");
var svg2 = merge([aria, svg, xlink, xmlns, xml], "svg");

// node_modules/space-separated-tokens/index.js
function stringify2(values) {
  return values.join(" ").trim();
}

// node_modules/inline-style-parser/esm/index.mjs
var COMMENT_REGEX = /\/\*(?:[^*]|\*(?!\/))*\*\//g;
var NEWLINE_REGEX = /\n/g;
var WHITESPACE_REGEX = /^\s*/;
var PROPERTY_REGEX = /^(\*?[-#/*\\\w]+(\[[0-9a-z_-]+\])?)\s*/;
var COLON_REGEX = /^:\s*/;
var VALUE_REGEX = /^((?:'(?:\\.|[^'\\])*'|"(?:\\.|[^"\\])*"|url\((?:'(?:\\.|[^'\\])*'|"(?:\\.|[^"\\])*"|[^)]*)\)|[^};])+)/;
var SEMICOLON_REGEX = /^[;\s]*/;
var NEWLINE = "\n";
var FORWARD_SLASH = "/";
var ASTERISK = "*";
var EMPTY_STRING = "";
var TYPE_COMMENT = "comment";
var TYPE_DECLARATION = "declaration";
function index(style, options) {
  if (typeof style !== "string") {
    throw new TypeError("First argument must be a string");
  }
  if (!style) return [];
  options = options || {};
  var lineno = 1;
  var column = 1;
  function updatePosition(str) {
    var lines = str.match(NEWLINE_REGEX);
    if (lines) lineno += lines.length;
    var i = str.lastIndexOf(NEWLINE);
    column = ~i ? str.length - i : column + str.length;
  }
  function position3() {
    var start2 = { line: lineno, column };
    return function(node2) {
      node2.position = new Position(start2);
      whitespace2();
      return node2;
    };
  }
  function Position(start2) {
    this.start = start2;
    this.end = { line: lineno, column };
    this.source = options.source;
  }
  Position.prototype.content = style;
  function error(msg) {
    var err = new Error(
      options.source + ":" + lineno + ":" + column + ": " + msg
    );
    err.reason = msg;
    err.filename = options.source;
    err.line = lineno;
    err.column = column;
    err.source = style;
    if (options.silent) ;
    else {
      throw err;
    }
  }
  function match(re2) {
    var m = re2.exec(style);
    if (!m) return;
    var str = m[0];
    updatePosition(str);
    style = style.slice(str.length);
    return m;
  }
  function whitespace2() {
    match(WHITESPACE_REGEX);
  }
  function comments(rules) {
    var c;
    while (c = comment()) {
      rules.push(c);
    }
    return rules;
  }
  function comment() {
    var pos = position3();
    if (FORWARD_SLASH != style.charAt(0) || ASTERISK != style.charAt(1)) return;
    var i = 2;
    while (EMPTY_STRING != style.charAt(i) && (ASTERISK != style.charAt(i) || FORWARD_SLASH != style.charAt(i + 1))) {
      ++i;
    }
    i += 2;
    if (EMPTY_STRING === style.charAt(i - 1)) {
      return error("End of comment missing");
    }
    var str = style.slice(2, i - 2);
    column += 2;
    updatePosition(str);
    style = style.slice(i);
    column += 2;
    return pos({
      type: TYPE_COMMENT,
      comment: str
    });
  }
  function declaration() {
    var pos = position3();
    var prop = match(PROPERTY_REGEX);
    if (!prop) return;
    comment();
    if (!match(COLON_REGEX)) return error("property missing ':'");
    var val = match(VALUE_REGEX);
    var ret = pos({
      type: TYPE_DECLARATION,
      property: prop[0].replace(COMMENT_REGEX, EMPTY_STRING).trim(),
      value: val ? val[0].replace(COMMENT_REGEX, EMPTY_STRING).trim() : EMPTY_STRING
    });
    match(SEMICOLON_REGEX);
    return ret;
  }
  function declarations() {
    var decls = [];
    comments(decls);
    var decl;
    while (decl = declaration()) {
      decls.push(decl);
      comments(decls);
    }
    return decls;
  }
  whitespace2();
  return declarations();
}

// node_modules/style-to-object/esm/index.mjs
function StyleToObject(style, iterator) {
  let styleObject = null;
  if (!style || typeof style !== "string") return styleObject;
  const declarations = index(style);
  const hasIterator = typeof iterator === "function";
  declarations.forEach((declaration) => {
    if (declaration.type !== "declaration") return;
    const { property, value } = declaration;
    if (hasIterator) iterator(property, value, declaration);
    else if (value) {
      styleObject = styleObject ?? {};
      styleObject[property] = value;
    }
  });
  return styleObject;
}

// node_modules/unist-util-position/lib/index.js
var pointEnd = point("end");
var pointStart = point("start");
function point(type) {
  return point4;
  function point4(node2) {
    const point5 = node2 && node2.position && node2.position[type] || {};
    if (typeof point5.line === "number" && point5.line > 0 && typeof point5.column === "number" && point5.column > 0) {
      return {
        line: point5.line,
        column: point5.column,
        offset: typeof point5.offset === "number" && point5.offset > -1 ? point5.offset : void 0
      };
    }
  }
}
function position(node2) {
  const start2 = pointStart(node2);
  const end = pointEnd(node2);
  if (start2 && end) {
    return { start: start2, end };
  }
}

// node_modules/unist-util-stringify-position/lib/index.js
function stringifyPosition(value) {
  if (!value || typeof value !== "object") {
    return "";
  }
  if ("position" in value || "type" in value) {
    return position2(value.position);
  }
  if ("start" in value || "end" in value) {
    return position2(value);
  }
  if ("line" in value || "column" in value) {
    return point2(value);
  }
  return "";
}
function point2(point4) {
  return index2(point4 && point4.line) + ":" + index2(point4 && point4.column);
}
function position2(pos) {
  return point2(pos && pos.start) + "-" + point2(pos && pos.end);
}
function index2(value) {
  return value && typeof value === "number" ? value : 1;
}

// node_modules/vfile-message/lib/index.js
var VFileMessage = class extends Error {
  /**
   * Create a message for `reason`.
   *
   * > 🪦 **Note**: also has obsolete signatures.
   *
   * @overload
   * @param {string} reason
   *   Reason for message, should use markdown.
   * @param {Options | null | undefined} [options]
   *   Configuration (optional).
   * @returns
   *   Instance of `VFileMessage`.
   *
   * @overload
   * @param {string} reason
   *   Reason for message, should use markdown.
   * @param {Node | NodeLike | null | undefined} parent
   *   Node or node-like, used for `ancestors` and `place`.
   * @param {string | null | undefined} [origin]
   *   Place in code where the message originates (optional).
   * @returns
   *   Instance of `VFileMessage`.
   *
   * @overload
   * @param {string} reason
   *   Reason for message, should use markdown.
   * @param {Point | Position | null | undefined} place
   *   Place of message.
   * @param {string | null | undefined} [origin]
   *   Place in code where the message originates (optional).
   * @returns
   *   Instance of `VFileMessage`.
   *
   * @overload
   * @param {string} reason
   *   Reason for message, should use markdown.
   * @param {string | null | undefined} [origin]
   *   Place in code where the message originates (optional).
   * @returns
   *   Instance of `VFileMessage`.
   *
   * @overload
   * @param {Error | VFileMessage} cause
   *   Original error cause of the message.
   * @param {Node | NodeLike | null | undefined} parent
   *   Node or node-like, used for `ancestors` and `place`.
   * @param {string | null | undefined} [origin]
   *   Place in code where the message originates (optional).
   * @returns
   *   Instance of `VFileMessage`.
   *
   * @overload
   * @param {Error | VFileMessage} cause
   *   Original error cause of the message.
   * @param {Point | Position | null | undefined} place
   *   Place of message.
   * @param {string | null | undefined} [origin]
   *   Place in code where the message originates (optional).
   * @returns
   *   Instance of `VFileMessage`.
   *
   * @overload
   * @param {Error | VFileMessage} cause
   *   Original error cause of the message.
   * @param {string | null | undefined} [origin]
   *   Place in code where the message originates (optional).
   * @returns
   *   Instance of `VFileMessage`.
   *
   * @param {Error | VFileMessage | string} causeOrReason
   *   Reason for message, should use markdown.
   * @param {Node | NodeLike | Options | Point | Position | string | null | undefined} [optionsOrParentOrPlace]
   *   Configuration (optional).
   * @param {string | null | undefined} [origin]
   *   Place in code where the message originates (example:
   *   `'my-package:my-rule'` or `'my-rule'`).
   * @returns
   *   Instance of `VFileMessage`.
   */
  // eslint-disable-next-line complexity
  constructor(causeOrReason, optionsOrParentOrPlace, origin) {
    super();
    if (typeof optionsOrParentOrPlace === "string") {
      origin = optionsOrParentOrPlace;
      optionsOrParentOrPlace = void 0;
    }
    let reason = "";
    let options = {};
    let legacyCause = false;
    if (optionsOrParentOrPlace) {
      if ("line" in optionsOrParentOrPlace && "column" in optionsOrParentOrPlace) {
        options = { place: optionsOrParentOrPlace };
      } else if ("start" in optionsOrParentOrPlace && "end" in optionsOrParentOrPlace) {
        options = { place: optionsOrParentOrPlace };
      } else if ("type" in optionsOrParentOrPlace) {
        options = {
          ancestors: [optionsOrParentOrPlace],
          place: optionsOrParentOrPlace.position
        };
      } else {
        options = { ...optionsOrParentOrPlace };
      }
    }
    if (typeof causeOrReason === "string") {
      reason = causeOrReason;
    } else if (!options.cause && causeOrReason) {
      legacyCause = true;
      reason = causeOrReason.message;
      options.cause = causeOrReason;
    }
    if (!options.ruleId && !options.source && typeof origin === "string") {
      const index3 = origin.indexOf(":");
      if (index3 === -1) {
        options.ruleId = origin;
      } else {
        options.source = origin.slice(0, index3);
        options.ruleId = origin.slice(index3 + 1);
      }
    }
    if (!options.place && options.ancestors) {
      const parent = options.ancestors[options.ancestors.length - 1];
      if (parent) {
        options.place = parent.position;
      }
    }
    const start2 = options.place && "start" in options.place ? options.place.start : options.place;
    this.ancestors = options.ancestors || void 0;
    this.cause = options.cause || void 0;
    this.column = start2 ? start2.column : void 0;
    this.fatal = void 0;
    this.file = "";
    this.message = reason;
    this.line = start2 ? start2.line : void 0;
    this.name = stringifyPosition(options.place) || "1:1";
    this.place = options.place || void 0;
    this.reason = this.message;
    this.ruleId = options.ruleId || void 0;
    this.source = options.source || void 0;
    this.stack = legacyCause && options.cause && typeof options.cause.stack === "string" ? options.cause.stack : "";
    this.actual = void 0;
    this.expected = void 0;
    this.note = void 0;
    this.url = void 0;
  }
};
Object.defineProperties(VFileMessage.prototype, {
  file: { configurable: true, enumerable: true, value: "", writable: true },
  name: { configurable: true, enumerable: true, value: "", writable: true },
  reason: { configurable: true, enumerable: true, value: "", writable: true },
  message: { configurable: true, enumerable: true, value: "", writable: true },
  stack: { configurable: true, enumerable: true, value: "", writable: true },
  column: { configurable: true, enumerable: true, writable: true },
  line: { configurable: true, enumerable: true, writable: true },
  ancestors: { configurable: true, enumerable: true, writable: true },
  cause: { configurable: true, enumerable: true, writable: true },
  fatal: { configurable: true, enumerable: true, writable: true },
  place: { configurable: true, enumerable: true, writable: true },
  ruleId: { configurable: true, enumerable: true, writable: true },
  source: { configurable: true, enumerable: true, writable: true }
});

// node_modules/hast-util-to-jsx-runtime/lib/index.js
var own = {}.hasOwnProperty;
var emptyMap = /* @__PURE__ */ new Map();
var cap2 = /[A-Z]/g;
var dashLetter = /-([a-z])/g;
var tableElements = /* @__PURE__ */ new Set(["table", "tbody", "thead", "tfoot", "tr"]);
var tableCellElement = /* @__PURE__ */ new Set(["td", "th"]);
var docs = "https://github.com/syntax-tree/hast-util-to-jsx-runtime";
function toJsxRuntime(tree, options) {
  if (!options || options.Fragment === void 0) {
    throw new TypeError("Expected `Fragment` in options");
  }
  const filePath = options.filePath || void 0;
  let create2;
  if (options.development) {
    if (typeof options.jsxDEV !== "function") {
      throw new TypeError(
        "Expected `jsxDEV` in options when `development: true`"
      );
    }
    create2 = developmentCreate(filePath, options.jsxDEV);
  } else {
    if (typeof options.jsx !== "function") {
      throw new TypeError("Expected `jsx` in production options");
    }
    if (typeof options.jsxs !== "function") {
      throw new TypeError("Expected `jsxs` in production options");
    }
    create2 = productionCreate(filePath, options.jsx, options.jsxs);
  }
  const state2 = {
    Fragment: options.Fragment,
    ancestors: [],
    components: options.components || {},
    create: create2,
    elementAttributeNameCase: options.elementAttributeNameCase || "react",
    evaluater: options.createEvaluater ? options.createEvaluater() : void 0,
    filePath,
    ignoreInvalidStyle: options.ignoreInvalidStyle || false,
    passKeys: options.passKeys !== false,
    passNode: options.passNode || false,
    schema: options.space === "svg" ? svg2 : html2,
    stylePropertyNameCase: options.stylePropertyNameCase || "dom",
    tableCellAlignToStyle: options.tableCellAlignToStyle !== false
  };
  const result = one(state2, tree, void 0);
  if (result && typeof result !== "string") {
    return result;
  }
  return state2.create(
    tree,
    state2.Fragment,
    { children: result || void 0 },
    void 0
  );
}
function one(state2, node2, key) {
  if (node2.type === "element") {
    return element(state2, node2, key);
  }
  if (node2.type === "mdxFlowExpression" || node2.type === "mdxTextExpression") {
    return mdxExpression(state2, node2);
  }
  if (node2.type === "mdxJsxFlowElement" || node2.type === "mdxJsxTextElement") {
    return mdxJsxElement(state2, node2, key);
  }
  if (node2.type === "mdxjsEsm") {
    return mdxEsm(state2, node2);
  }
  if (node2.type === "root") {
    return root(state2, node2, key);
  }
  if (node2.type === "text") {
    return text(state2, node2);
  }
}
function element(state2, node2, key) {
  const parentSchema = state2.schema;
  let schema = parentSchema;
  if (node2.tagName.toLowerCase() === "svg" && parentSchema.space === "html") {
    schema = svg2;
    state2.schema = schema;
  }
  state2.ancestors.push(node2);
  const type = findComponentFromName(state2, node2.tagName, false);
  const props = createElementProps(state2, node2);
  let children = createChildren(state2, node2);
  if (tableElements.has(node2.tagName)) {
    children = children.filter(function(child) {
      return typeof child === "string" ? !whitespace(child) : true;
    });
  }
  addNode(state2, props, type, node2);
  addChildren(props, children);
  state2.ancestors.pop();
  state2.schema = parentSchema;
  return state2.create(node2, type, props, key);
}
function mdxExpression(state2, node2) {
  if (node2.data && node2.data.estree && state2.evaluater) {
    const program = node2.data.estree;
    const expression = program.body[0];
    ok(expression.type === "ExpressionStatement");
    return (
      /** @type {Child | undefined} */
      state2.evaluater.evaluateExpression(expression.expression)
    );
  }
  crashEstree(state2, node2.position);
}
function mdxEsm(state2, node2) {
  if (node2.data && node2.data.estree && state2.evaluater) {
    return (
      /** @type {Child | undefined} */
      state2.evaluater.evaluateProgram(node2.data.estree)
    );
  }
  crashEstree(state2, node2.position);
}
function mdxJsxElement(state2, node2, key) {
  const parentSchema = state2.schema;
  let schema = parentSchema;
  if (node2.name === "svg" && parentSchema.space === "html") {
    schema = svg2;
    state2.schema = schema;
  }
  state2.ancestors.push(node2);
  const type = node2.name === null ? state2.Fragment : findComponentFromName(state2, node2.name, true);
  const props = createJsxElementProps(state2, node2);
  const children = createChildren(state2, node2);
  addNode(state2, props, type, node2);
  addChildren(props, children);
  state2.ancestors.pop();
  state2.schema = parentSchema;
  return state2.create(node2, type, props, key);
}
function root(state2, node2, key) {
  const props = {};
  addChildren(props, createChildren(state2, node2));
  return state2.create(node2, state2.Fragment, props, key);
}
function text(_, node2) {
  return node2.value;
}
function addNode(state2, props, type, node2) {
  if (typeof type !== "string" && type !== state2.Fragment && state2.passNode) {
    props.node = node2;
  }
}
function addChildren(props, children) {
  if (children.length > 0) {
    const value = children.length > 1 ? children : children[0];
    if (value) {
      props.children = value;
    }
  }
}
function productionCreate(_, jsx11, jsxs9) {
  return create2;
  function create2(_2, type, props, key) {
    const isStaticChildren = Array.isArray(props.children);
    const fn = isStaticChildren ? jsxs9 : jsx11;
    return key ? fn(type, props, key) : fn(type, props);
  }
}
function developmentCreate(filePath, jsxDEV) {
  return create2;
  function create2(node2, type, props, key) {
    const isStaticChildren = Array.isArray(props.children);
    const point4 = pointStart(node2);
    return jsxDEV(
      type,
      props,
      key,
      isStaticChildren,
      {
        columnNumber: point4 ? point4.column - 1 : void 0,
        fileName: filePath,
        lineNumber: point4 ? point4.line : void 0
      },
      void 0
    );
  }
}
function createElementProps(state2, node2) {
  const props = {};
  let alignValue;
  let prop;
  for (prop in node2.properties) {
    if (prop !== "children" && own.call(node2.properties, prop)) {
      const result = createProperty(state2, prop, node2.properties[prop]);
      if (result) {
        const [key, value] = result;
        if (state2.tableCellAlignToStyle && key === "align" && typeof value === "string" && tableCellElement.has(node2.tagName)) {
          alignValue = value;
        } else {
          props[key] = value;
        }
      }
    }
  }
  if (alignValue) {
    const style = (
      /** @type {Style} */
      props.style || (props.style = {})
    );
    style[state2.stylePropertyNameCase === "css" ? "text-align" : "textAlign"] = alignValue;
  }
  return props;
}
function createJsxElementProps(state2, node2) {
  const props = {};
  for (const attribute of node2.attributes) {
    if (attribute.type === "mdxJsxExpressionAttribute") {
      if (attribute.data && attribute.data.estree && state2.evaluater) {
        const program = attribute.data.estree;
        const expression = program.body[0];
        ok(expression.type === "ExpressionStatement");
        const objectExpression = expression.expression;
        ok(objectExpression.type === "ObjectExpression");
        const property = objectExpression.properties[0];
        ok(property.type === "SpreadElement");
        Object.assign(
          props,
          state2.evaluater.evaluateExpression(property.argument)
        );
      } else {
        crashEstree(state2, node2.position);
      }
    } else {
      const { name: name2 } = attribute;
      let value;
      if (attribute.value && typeof attribute.value === "object") {
        if (attribute.value.data && attribute.value.data.estree && state2.evaluater) {
          const program = attribute.value.data.estree;
          const expression = program.body[0];
          ok(expression.type === "ExpressionStatement");
          value = state2.evaluater.evaluateExpression(expression.expression);
        } else {
          crashEstree(state2, node2.position);
        }
      } else {
        value = attribute.value === null ? true : attribute.value;
      }
      props[name2] = /** @type {Props[keyof Props]} */
      value;
    }
  }
  return props;
}
function createChildren(state2, node2) {
  const children = [];
  let index3 = -1;
  const countsByName = state2.passKeys ? /* @__PURE__ */ new Map() : emptyMap;
  while (++index3 < node2.children.length) {
    const child = node2.children[index3];
    let key;
    if (state2.passKeys) {
      const name2 = child.type === "element" ? child.tagName : child.type === "mdxJsxFlowElement" || child.type === "mdxJsxTextElement" ? child.name : void 0;
      if (name2) {
        const count = countsByName.get(name2) || 0;
        key = name2 + "-" + count;
        countsByName.set(name2, count + 1);
      }
    }
    const result = one(state2, child, key);
    if (result !== void 0) children.push(result);
  }
  return children;
}
function createProperty(state2, prop, value) {
  if (value === null || value === void 0 || typeof value === "number" && Number.isNaN(value)) {
    return;
  }
  const info = find(state2.schema, prop);
  if (Array.isArray(value)) {
    value = info.commaSeparated ? stringify(value) : stringify2(value);
  }
  if (info.property === "style") {
    const styleObject = typeof value === "object" ? state2.stylePropertyNameCase === "dom" ? value : transformStylesToCssCasing(value) : parseStyle(state2, String(value));
    return ["style", styleObject];
  }
  return [
    state2.elementAttributeNameCase === "react" && info.space ? hastToReact[info.property] || info.property : info.attribute,
    value
  ];
}
function parseStyle(state2, value) {
  const result = {};
  try {
    StyleToObject(value, function(property, value2) {
      if (property && value2) {
        let name2 = property;
        if (name2.slice(0, 2) !== "--") {
          name2 = name2.toLowerCase();
          if (state2.stylePropertyNameCase === "dom") {
            if (name2.slice(0, 4) === "-ms-") name2 = name2.slice(1);
            name2 = name2.replace(dashLetter, toCamel);
          }
        }
        result[name2] = value2;
      }
    });
  } catch (error) {
    if (state2.ignoreInvalidStyle) {
      return {};
    }
    const cause = (
      /** @type {Error} */
      error
    );
    const message = new VFileMessage("Cannot parse `style` attribute", {
      ancestors: state2.ancestors,
      cause,
      ruleId: "style",
      source: "hast-util-to-jsx-runtime"
    });
    message.file = state2.filePath || void 0;
    message.url = docs + "#cannot-parse-style-attribute";
    throw message;
  }
  return result;
}
function findComponentFromName(state2, name2, allowExpression) {
  let result;
  if (!allowExpression) {
    result = { type: "Literal", value: name2 };
  } else if (name2.includes(".")) {
    const identifiers = name2.split(".");
    let index3 = -1;
    let node2;
    while (++index3 < identifiers.length) {
      const prop = name(identifiers[index3]) ? { type: "Identifier", name: identifiers[index3] } : { type: "Literal", value: identifiers[index3] };
      node2 = node2 ? {
        type: "MemberExpression",
        object: node2,
        property: prop,
        computed: Boolean(index3 && prop.type === "Literal"),
        optional: false
      } : prop;
    }
    ok(node2, "always a result");
    result = node2;
  } else {
    result = name(name2) && !/^[a-z]/.test(name2) ? { type: "Identifier", name: name2 } : { type: "Literal", value: name2 };
  }
  if (result.type === "Literal") {
    const name3 = (
      /** @type {string | number} */
      result.value
    );
    return own.call(state2.components, name3) ? state2.components[name3] : name3;
  }
  if (state2.evaluater) {
    return state2.evaluater.evaluateExpression(result);
  }
  crashEstree(state2);
}
function crashEstree(state2, place2) {
  const message = new VFileMessage(
    "Cannot handle MDX estrees without `createEvaluater`",
    {
      ancestors: state2.ancestors,
      place: place2,
      ruleId: "mdx-estree",
      source: "hast-util-to-jsx-runtime"
    }
  );
  message.file = state2.filePath || void 0;
  message.url = docs + "#cannot-handle-mdx-estrees-without-createevaluater";
  throw message;
}
function transformStylesToCssCasing(domCasing) {
  const cssCasing = {};
  let from;
  for (from in domCasing) {
    if (own.call(domCasing, from)) {
      cssCasing[transformStyleToCssCasing(from)] = domCasing[from];
    }
  }
  return cssCasing;
}
function transformStyleToCssCasing(from) {
  if (from.slice(0, 2) === "--") return from;
  let to = from.replace(cap2, toDash);
  if (to.slice(0, 3) === "ms-") to = "-" + to;
  return to;
}
function toDash($0) {
  return "-" + $0.toLowerCase();
}
function toCamel(_, $1) {
  return $1.toUpperCase();
}

// node_modules/html-url-attributes/lib/index.js
var urlAttributes = {
  action: ["form"],
  cite: ["blockquote", "del", "ins", "q"],
  data: ["object"],
  formAction: ["button", "input"],
  href: ["a", "area", "base", "link"],
  icon: ["menuitem"],
  itemId: null,
  manifest: ["html"],
  ping: ["a", "area"],
  poster: ["video"],
  src: [
    "audio",
    "embed",
    "iframe",
    "img",
    "input",
    "script",
    "source",
    "track",
    "video"
  ]
};

// node_modules/react-markdown/lib/index.js
import { Fragment as Fragment2, jsx as jsx3, jsxs as jsxs2 } from "react/jsx-runtime";
import { useEffect as useEffect3, useState as useState3 } from "react";

// node_modules/mdast-util-to-string/lib/index.js
var emptyOptions2 = {};
function toString(value, options) {
  const settings = options || emptyOptions2;
  const includeImageAlt = typeof settings.includeImageAlt === "boolean" ? settings.includeImageAlt : true;
  const includeHtml = typeof settings.includeHtml === "boolean" ? settings.includeHtml : true;
  return one2(value, includeImageAlt, includeHtml);
}
function one2(value, includeImageAlt, includeHtml) {
  if (node(value)) {
    if ("value" in value) {
      return value.type === "html" && !includeHtml ? "" : value.value;
    }
    if (includeImageAlt && "alt" in value && value.alt) {
      return value.alt;
    }
    if ("children" in value) {
      return all(value.children, includeImageAlt, includeHtml);
    }
  }
  if (Array.isArray(value)) {
    return all(value, includeImageAlt, includeHtml);
  }
  return "";
}
function all(values, includeImageAlt, includeHtml) {
  const result = [];
  let index3 = -1;
  while (++index3 < values.length) {
    result[index3] = one2(values[index3], includeImageAlt, includeHtml);
  }
  return result.join("");
}
function node(value) {
  return Boolean(value && typeof value === "object");
}

// node_modules/decode-named-character-reference/index.dom.js
var element2 = document.createElement("i");
function decodeNamedCharacterReference(value) {
  const characterReference2 = "&" + value + ";";
  element2.innerHTML = characterReference2;
  const character = element2.textContent;
  if (character.charCodeAt(character.length - 1) === 59 && value !== "semi") {
    return false;
  }
  return character === characterReference2 ? false : character;
}

// node_modules/micromark-util-chunked/index.js
function splice(list4, start2, remove, items) {
  const end = list4.length;
  let chunkStart = 0;
  let parameters;
  if (start2 < 0) {
    start2 = -start2 > end ? 0 : end + start2;
  } else {
    start2 = start2 > end ? end : start2;
  }
  remove = remove > 0 ? remove : 0;
  if (items.length < 1e4) {
    parameters = Array.from(items);
    parameters.unshift(start2, remove);
    list4.splice(...parameters);
  } else {
    if (remove) list4.splice(start2, remove);
    while (chunkStart < items.length) {
      parameters = items.slice(chunkStart, chunkStart + 1e4);
      parameters.unshift(start2, 0);
      list4.splice(...parameters);
      chunkStart += 1e4;
      start2 += 1e4;
    }
  }
}
function push(list4, items) {
  if (list4.length > 0) {
    splice(list4, list4.length, 0, items);
    return list4;
  }
  return items;
}

// node_modules/micromark-util-combine-extensions/index.js
var hasOwnProperty = {}.hasOwnProperty;
function combineExtensions(extensions) {
  const all2 = {};
  let index3 = -1;
  while (++index3 < extensions.length) {
    syntaxExtension(all2, extensions[index3]);
  }
  return all2;
}
function syntaxExtension(all2, extension2) {
  let hook;
  for (hook in extension2) {
    const maybe = hasOwnProperty.call(all2, hook) ? all2[hook] : void 0;
    const left = maybe || (all2[hook] = {});
    const right = extension2[hook];
    let code4;
    if (right) {
      for (code4 in right) {
        if (!hasOwnProperty.call(left, code4)) left[code4] = [];
        const value = right[code4];
        constructs(
          // @ts-expect-error Looks like a list.
          left[code4],
          Array.isArray(value) ? value : value ? [value] : []
        );
      }
    }
  }
}
function constructs(existing, list4) {
  let index3 = -1;
  const before = [];
  while (++index3 < list4.length) {
    ;
    (list4[index3].add === "after" ? existing : before).push(list4[index3]);
  }
  splice(existing, 0, 0, before);
}

// node_modules/micromark-util-decode-numeric-character-reference/index.js
function decodeNumericCharacterReference(value, base) {
  const code4 = Number.parseInt(value, base);
  if (
    // C0 except for HT, LF, FF, CR, space.
    code4 < 9 || code4 === 11 || code4 > 13 && code4 < 32 || // Control character (DEL) of C0, and C1 controls.
    code4 > 126 && code4 < 160 || // Lone high surrogates and low surrogates.
    code4 > 55295 && code4 < 57344 || // Noncharacters.
    code4 > 64975 && code4 < 65008 || /* eslint-disable no-bitwise */
    (code4 & 65535) === 65535 || (code4 & 65535) === 65534 || /* eslint-enable no-bitwise */
    // Out of range
    code4 > 1114111
  ) {
    return "\uFFFD";
  }
  return String.fromCodePoint(code4);
}

// node_modules/micromark-util-normalize-identifier/index.js
function normalizeIdentifier(value) {
  return value.replace(/[\t\n\r ]+/g, " ").replace(/^ | $/g, "").toLowerCase().toUpperCase();
}

// node_modules/micromark-util-character/index.js
var asciiAlpha = regexCheck(/[A-Za-z]/);
var asciiAlphanumeric = regexCheck(/[\dA-Za-z]/);
var asciiAtext = regexCheck(/[#-'*+\--9=?A-Z^-~]/);
function asciiControl(code4) {
  return (
    // Special whitespace codes (which have negative values), C0 and Control
    // character DEL
    code4 !== null && (code4 < 32 || code4 === 127)
  );
}
var asciiDigit = regexCheck(/\d/);
var asciiHexDigit = regexCheck(/[\dA-Fa-f]/);
var asciiPunctuation = regexCheck(/[!-/:-@[-`{-~]/);
function markdownLineEnding(code4) {
  return code4 !== null && code4 < -2;
}
function markdownLineEndingOrSpace(code4) {
  return code4 !== null && (code4 < 0 || code4 === 32);
}
function markdownSpace(code4) {
  return code4 === -2 || code4 === -1 || code4 === 32;
}
var unicodePunctuation = regexCheck(/\p{P}|\p{S}/u);
var unicodeWhitespace = regexCheck(/\s/);
function regexCheck(regex) {
  return check2;
  function check2(code4) {
    return code4 !== null && code4 > -1 && regex.test(String.fromCharCode(code4));
  }
}

// node_modules/micromark-util-sanitize-uri/index.js
function normalizeUri(value) {
  const result = [];
  let index3 = -1;
  let start2 = 0;
  let skip = 0;
  while (++index3 < value.length) {
    const code4 = value.charCodeAt(index3);
    let replace2 = "";
    if (code4 === 37 && asciiAlphanumeric(value.charCodeAt(index3 + 1)) && asciiAlphanumeric(value.charCodeAt(index3 + 2))) {
      skip = 2;
    } else if (code4 < 128) {
      if (!/[!#$&-;=?-Z_a-z~]/.test(String.fromCharCode(code4))) {
        replace2 = String.fromCharCode(code4);
      }
    } else if (code4 > 55295 && code4 < 57344) {
      const next = value.charCodeAt(index3 + 1);
      if (code4 < 56320 && next > 56319 && next < 57344) {
        replace2 = String.fromCharCode(code4, next);
        skip = 1;
      } else {
        replace2 = "\uFFFD";
      }
    } else {
      replace2 = String.fromCharCode(code4);
    }
    if (replace2) {
      result.push(value.slice(start2, index3), encodeURIComponent(replace2));
      start2 = index3 + skip + 1;
      replace2 = "";
    }
    if (skip) {
      index3 += skip;
      skip = 0;
    }
  }
  return result.join("") + value.slice(start2);
}

// node_modules/micromark-factory-space/index.js
function factorySpace(effects, ok3, type, max) {
  const limit = max ? max - 1 : Infinity;
  let size = 0;
  return start2;
  function start2(code4) {
    if (markdownSpace(code4)) {
      effects.enter(type);
      return prefix(code4);
    }
    return ok3(code4);
  }
  function prefix(code4) {
    if (markdownSpace(code4) && size++ < limit) {
      effects.consume(code4);
      return prefix;
    }
    effects.exit(type);
    return ok3(code4);
  }
}
function factorySpaceMinMax(effects, ok3, nok, type, min, max) {
  let size = 0;
  return start2;
  function start2(code4) {
    if (max > 0 && markdownSpace(code4)) {
      effects.enter(type);
      return prefix(code4);
    }
    return after(code4);
  }
  function prefix(code4) {
    if (markdownSpace(code4) && size < max) {
      effects.consume(code4);
      size++;
      return prefix;
    }
    effects.exit(type);
    return after(code4);
  }
  function after(code4) {
    return size >= min ? ok3(code4) : nok(code4);
  }
}

// node_modules/micromark/lib/initialize/content.js
var content = {
  tokenize: initializeContent
};
function initializeContent(effects) {
  const contentStart = effects.attempt(this.parser.constructs.contentInitial, afterContentStartConstruct, paragraphInitial);
  let previous3;
  return contentStart;
  function afterContentStartConstruct(code4) {
    if (code4 === null) {
      effects.consume(code4);
      return;
    }
    effects.enter("lineEnding");
    effects.consume(code4);
    effects.exit("lineEnding");
    return factorySpace(effects, contentStart, "linePrefix");
  }
  function paragraphInitial(code4) {
    effects.enter("paragraph");
    return lineStart(code4);
  }
  function lineStart(code4) {
    const token = effects.enter("chunkText", {
      contentType: "text",
      previous: previous3
    });
    if (previous3) {
      previous3.next = token;
    }
    previous3 = token;
    return data(code4);
  }
  function data(code4) {
    if (code4 === null) {
      effects.exit("chunkText");
      effects.exit("paragraph");
      effects.consume(code4);
      return;
    }
    if (markdownLineEnding(code4)) {
      effects.consume(code4);
      effects.exit("chunkText");
      return lineStart;
    }
    effects.consume(code4);
    return data;
  }
}

// node_modules/micromark-util-edit-map/index.js
var EditMap = class {
  /**
   * Create a new edit map.
   *
   * @returns
   *   New instance.
   */
  constructor() {
    this.index = /* @__PURE__ */ new Map();
    this.map = [];
  }
  /**
   * Create an edit: a remove and/or add at a certain place.
   *
   * @param {number} index
   *   Index at which to apply the edit.
   * @param {number} remove
   *   Count of items to remove at the index.
   * @param {Array<Event>} add
   *   Items to add at the index.
   * @returns {undefined}
   *   Nothing.
   */
  add(index3, remove, add) {
    addImplementation(this, index3, remove, add, false);
  }
  /**
   * Create an edit: but insert `add` before existing additions, instead of
   * after them.
   *
   * @param {number} index
   *   Index at which to apply the edit.
   * @param {number} remove
   *   Count of items to remove at the index.
   * @param {Array<Event>} add
   *   Items to add at the index.
   * @returns {undefined}
   *   Nothing.
   */
  addBefore(index3, remove, add) {
    addImplementation(this, index3, remove, add, true);
  }
  /**
   * Done, change the events.
   *
   * @param {Array<Event>} events
   *   List of events to apply the edits to.
   * @returns {undefined}
   *   Nothing.
   */
  consume(events) {
    this.map.sort(function(a, b) {
      return a[0] - b[0];
    });
    if (this.map.length === 0) {
      return;
    }
    let index3 = this.map.length;
    const vecs = [];
    while (index3 > 0) {
      index3 -= 1;
      vecs.push(events.slice(this.map[index3][0] + this.map[index3][1]), this.map[index3][2]);
      events.length = this.map[index3][0];
    }
    vecs.push(events.slice());
    events.length = 0;
    let slice = vecs.pop();
    while (slice) {
      for (const element3 of slice) {
        events.push(element3);
      }
      slice = vecs.pop();
    }
    this.map.length = 0;
    this.index.clear();
  }
};
function addImplementation(editMap, at, remove, add, before) {
  if (remove === 0 && add.length === 0) {
    return;
  }
  const existing = editMap.index.get(at);
  if (existing) {
    existing[1] += remove;
    if (before) {
      add.push(...existing[2]);
      existing[2] = add;
    } else {
      existing[2].push(...add);
    }
    return;
  }
  const change = [at, remove, add];
  editMap.map.push(change);
  editMap.index.set(at, change);
}

// node_modules/micromark/lib/initialize/document.js
var document2 = {
  tokenize: initializeDocument
};
var containerConstruct = {
  tokenize: tokenizeContainer
};
function initializeDocument(effects) {
  const self2 = this;
  const stack = [];
  let continued = 0;
  let childFlow;
  let childToken;
  let lineStartOffset;
  return start2;
  function start2(code4) {
    if (continued < stack.length) {
      const item = stack[continued];
      self2.containerState = item[1];
      return effects.attempt(item[0].continuation, documentContinue, checkNewContainers)(code4);
    }
    return checkNewContainers(code4);
  }
  function documentContinue(code4) {
    continued++;
    if (self2.containerState._closeFlow) {
      self2.containerState._closeFlow = void 0;
      if (childFlow) {
        closeFlow();
      }
      const indexBeforeExits = self2.events.length;
      let indexBeforeFlow = indexBeforeExits;
      let point4;
      while (indexBeforeFlow--) {
        if (self2.events[indexBeforeFlow][0] === "exit" && self2.events[indexBeforeFlow][1].type === "chunkFlow") {
          point4 = self2.events[indexBeforeFlow][1].end;
          break;
        }
      }
      exitContainers(continued);
      let index3 = indexBeforeExits;
      while (index3 < self2.events.length) {
        self2.events[index3][1].end = {
          ...point4
        };
        index3++;
      }
      const editMap = new EditMap();
      editMap.add(indexBeforeFlow + 1, 0, self2.events.slice(indexBeforeExits));
      editMap.add(indexBeforeExits, index3 - indexBeforeExits, []);
      editMap.consume(self2.events);
      return checkNewContainers(code4);
    }
    return start2(code4);
  }
  function checkNewContainers(code4) {
    if (continued === stack.length) {
      if (!childFlow) {
        return documentContinued(code4);
      }
      if (childFlow.currentConstruct && childFlow.currentConstruct.concrete) {
        return flowStart(code4);
      }
      self2.interrupt = Boolean(childFlow.currentConstruct && !childFlow._gfmTableDynamicInterruptHack);
    }
    self2.containerState = {};
    return effects.check(containerConstruct, thereIsANewContainer, thereIsNoNewContainer)(code4);
  }
  function thereIsANewContainer(code4) {
    if (childFlow) {
      closeFlow();
    }
    exitContainers(continued);
    return documentContinued(code4);
  }
  function thereIsNoNewContainer(code4) {
    self2.parser.lazy[self2.now().line] = continued !== stack.length;
    lineStartOffset = self2.now().offset;
    return flowStart(code4);
  }
  function documentContinued(code4) {
    self2.containerState = {};
    return effects.attempt(containerConstruct, containerContinue, flowStart)(code4);
  }
  function containerContinue(code4) {
    continued++;
    stack.push([self2.currentConstruct, self2.containerState]);
    return documentContinued(code4);
  }
  function flowStart(code4) {
    if (code4 === null) {
      if (childFlow) {
        closeFlow();
      }
      exitContainers(0);
      effects.consume(code4);
      return;
    }
    childFlow = childFlow || self2.parser.flow(self2.now());
    effects.enter("chunkFlow", {
      _tokenizer: childFlow,
      contentType: "flow",
      previous: childToken
    });
    return flowContinue(code4);
  }
  function flowContinue(code4) {
    if (code4 === null) {
      writeToChild(effects.exit("chunkFlow"), true);
      exitContainers(0);
      effects.consume(code4);
      return;
    }
    if (markdownLineEnding(code4)) {
      effects.consume(code4);
      writeToChild(effects.exit("chunkFlow"));
      continued = 0;
      self2.interrupt = void 0;
      return start2;
    }
    effects.consume(code4);
    return flowContinue;
  }
  function writeToChild(token, endOfFile) {
    const stream = self2.sliceStream(token);
    if (endOfFile) {
      stream.push(null);
    }
    token.previous = childToken;
    if (childToken) {
      childToken.next = token;
    }
    childToken = token;
    childFlow.defineSkip(token.start);
    childFlow.write(stream);
    if (self2.parser.lazy[token.start.line]) {
      let index3 = childFlow.events.length;
      while (index3--) {
        if (
          // The token starts before the line ending…
          childFlow.events[index3][1].start.offset < lineStartOffset && // …and either is not ended yet…
          (!childFlow.events[index3][1].end || // …or ends after it.
          childFlow.events[index3][1].end.offset > lineStartOffset)
        ) {
          return;
        }
      }
      const indexBeforeExits = self2.events.length;
      let indexBeforeFlow = indexBeforeExits;
      let seen;
      let point4;
      while (indexBeforeFlow--) {
        if (self2.events[indexBeforeFlow][0] === "exit" && self2.events[indexBeforeFlow][1].type === "chunkFlow") {
          if (seen) {
            point4 = self2.events[indexBeforeFlow][1].end;
            break;
          }
          seen = true;
        }
      }
      exitContainers(continued);
      index3 = indexBeforeExits;
      while (index3 < self2.events.length) {
        self2.events[index3][1].end = {
          ...point4
        };
        index3++;
      }
      const editMap = new EditMap();
      editMap.add(indexBeforeFlow + 1, 0, self2.events.slice(indexBeforeExits));
      editMap.add(indexBeforeExits, index3 - indexBeforeExits, []);
      editMap.consume(self2.events);
    }
  }
  function exitContainers(size) {
    let index3 = stack.length;
    while (index3-- > size) {
      const entry = stack[index3];
      self2.containerState = entry[1];
      entry[0].exit.call(self2, effects);
    }
    stack.length = size;
  }
  function closeFlow() {
    childFlow.write([null]);
    childToken = void 0;
    childFlow = void 0;
    self2.containerState._closeFlow = void 0;
  }
}
function tokenizeContainer(effects, ok3, nok) {
  return factorySpace(effects, effects.attempt(this.parser.constructs.document, ok3, nok), "linePrefix", this.parser.constructs.disable.null.includes("codeIndented") ? void 0 : 4);
}

// node_modules/micromark-util-classify-character/index.js
function classifyCharacter(code4) {
  if (code4 === null || markdownLineEndingOrSpace(code4) || unicodeWhitespace(code4)) {
    return 1;
  }
  if (unicodePunctuation(code4)) {
    return 2;
  }
}

// node_modules/micromark-util-resolve-all/index.js
function resolveAll(constructs2, events, context) {
  const called = [];
  let index3 = -1;
  while (++index3 < constructs2.length) {
    const resolve = constructs2[index3].resolveAll;
    if (resolve && !called.includes(resolve)) {
      events = resolve(events, context);
      called.push(resolve);
    }
  }
  return events;
}

// node_modules/micromark-core-commonmark/lib/attention.js
var attention = {
  name: "attention",
  resolveAll: resolveAllAttention,
  tokenize: tokenizeAttention
};
function resolveAllAttention(events, context) {
  let index3 = -1;
  let nextEvents;
  while (++index3 < events.length) {
    if (events[index3][0] === "enter" && events[index3][1].type === "attentionSequence" && events[index3][1]._close) {
      let open = index3;
      while (open--) {
        if (events[open][0] === "exit" && events[open][1].type === "attentionSequence" && events[open][1]._open && // If the markers are the same:
        context.sliceSerialize(events[open][1]).charCodeAt(0) === context.sliceSerialize(events[index3][1]).charCodeAt(0)) {
          if ((events[open][1]._close || events[index3][1]._open) && (events[index3][1].end.offset - events[index3][1].start.offset) % 3 && !((events[open][1].end.offset - events[open][1].start.offset + events[index3][1].end.offset - events[index3][1].start.offset) % 3)) {
            continue;
          }
          const use = events[open][1].end.offset - events[open][1].start.offset > 1 && events[index3][1].end.offset - events[index3][1].start.offset > 1 ? 2 : 1;
          const start2 = {
            ...events[open][1].end
          };
          const end = {
            ...events[index3][1].start
          };
          movePoint(start2, -use);
          movePoint(end, use);
          const openingSequence = {
            type: use > 1 ? "strongSequence" : "emphasisSequence",
            start: start2,
            end: {
              ...events[open][1].end
            }
          };
          const closingSequence = {
            type: use > 1 ? "strongSequence" : "emphasisSequence",
            start: {
              ...events[index3][1].start
            },
            end
          };
          const text7 = {
            type: use > 1 ? "strongText" : "emphasisText",
            start: {
              ...events[open][1].end
            },
            end: {
              ...events[index3][1].start
            }
          };
          const group = {
            type: use > 1 ? "strong" : "emphasis",
            start: {
              ...openingSequence.start
            },
            end: {
              ...closingSequence.end
            }
          };
          events[open][1].end = {
            ...openingSequence.start
          };
          events[index3][1].start = {
            ...closingSequence.end
          };
          nextEvents = [];
          if (events[open][1].end.offset - events[open][1].start.offset) {
            nextEvents = push(nextEvents, [["enter", events[open][1], context], ["exit", events[open][1], context]]);
          }
          nextEvents = push(nextEvents, [["enter", group, context], ["enter", openingSequence, context], ["exit", openingSequence, context], ["enter", text7, context]]);
          nextEvents = push(nextEvents, resolveAll(context.parser.constructs.insideSpan.null, events.slice(open + 1, index3), context));
          nextEvents = push(nextEvents, [["exit", text7, context], ["enter", closingSequence, context], ["exit", closingSequence, context], ["exit", group, context]]);
          let offset = 0;
          if (events[index3][1].end.offset - events[index3][1].start.offset) {
            offset = 2;
            nextEvents = push(nextEvents, [["enter", events[index3][1], context], ["exit", events[index3][1], context]]);
          }
          splice(events, open - 1, index3 - open + 3, nextEvents);
          index3 = open + nextEvents.length - offset - 2;
          break;
        }
      }
    }
  }
  index3 = -1;
  while (++index3 < events.length) {
    if (events[index3][1].type === "attentionSequence") {
      events[index3][1].type = "data";
    }
  }
  return events;
}
function tokenizeAttention(effects, ok3) {
  const attentionMarkers2 = this.parser.constructs.attentionMarkers.null;
  const previous3 = this.previous;
  const before = classifyCharacter(previous3);
  let marker;
  return start2;
  function start2(code4) {
    marker = code4;
    effects.enter("attentionSequence");
    return inside(code4);
  }
  function inside(code4) {
    if (code4 === marker) {
      effects.consume(code4);
      return inside;
    }
    const token = effects.exit("attentionSequence");
    const after = classifyCharacter(code4);
    const open = !after || after === 2 && before || attentionMarkers2.includes(code4) && code4 !== 42 && code4 !== 95;
    const close = !before || before === 2 && after || attentionMarkers2.includes(previous3) && previous3 !== 42 && previous3 !== 95;
    token._open = Boolean(marker === 42 ? open : open && (before || !close));
    token._close = Boolean(marker === 42 ? close : close && (after || !open));
    return ok3(code4);
  }
}
function movePoint(point4, offset) {
  point4.column += offset;
  point4.offset += offset;
  point4._bufferIndex += offset;
}

// node_modules/micromark-core-commonmark/lib/autolink.js
var autolink = {
  name: "autolink",
  tokenize: tokenizeAutolink
};
function tokenizeAutolink(effects, ok3, nok) {
  let size = 0;
  return start2;
  function start2(code4) {
    effects.enter("autolink");
    effects.enter("autolinkMarker");
    effects.consume(code4);
    effects.exit("autolinkMarker");
    effects.enter("autolinkProtocol");
    return open;
  }
  function open(code4) {
    if (asciiAlpha(code4)) {
      effects.consume(code4);
      return schemeOrEmailAtext;
    }
    if (code4 === 64) {
      return nok(code4);
    }
    return emailAtext(code4);
  }
  function schemeOrEmailAtext(code4) {
    if (code4 === 43 || code4 === 45 || code4 === 46 || asciiAlphanumeric(code4)) {
      size = 1;
      return schemeInsideOrEmailAtext(code4);
    }
    return emailAtext(code4);
  }
  function schemeInsideOrEmailAtext(code4) {
    if (code4 === 58) {
      effects.consume(code4);
      size = 0;
      return urlInside;
    }
    if ((code4 === 43 || code4 === 45 || code4 === 46 || asciiAlphanumeric(code4)) && size++ < 32) {
      effects.consume(code4);
      return schemeInsideOrEmailAtext;
    }
    size = 0;
    return emailAtext(code4);
  }
  function urlInside(code4) {
    if (code4 === 62) {
      effects.exit("autolinkProtocol");
      effects.enter("autolinkMarker");
      effects.consume(code4);
      effects.exit("autolinkMarker");
      effects.exit("autolink");
      return ok3;
    }
    if (code4 === null || code4 === 32 || code4 === 60 || asciiControl(code4)) {
      return nok(code4);
    }
    effects.consume(code4);
    return urlInside;
  }
  function emailAtext(code4) {
    if (code4 === 64) {
      effects.consume(code4);
      return emailAtSignOrDot;
    }
    if (asciiAtext(code4)) {
      effects.consume(code4);
      return emailAtext;
    }
    return nok(code4);
  }
  function emailAtSignOrDot(code4) {
    return asciiAlphanumeric(code4) ? emailLabel(code4) : nok(code4);
  }
  function emailLabel(code4) {
    if (code4 === 46) {
      effects.consume(code4);
      size = 0;
      return emailAtSignOrDot;
    }
    if (code4 === 62) {
      effects.exit("autolinkProtocol").type = "autolinkEmail";
      effects.enter("autolinkMarker");
      effects.consume(code4);
      effects.exit("autolinkMarker");
      effects.exit("autolink");
      return ok3;
    }
    return emailValue(code4);
  }
  function emailValue(code4) {
    if ((code4 === 45 || asciiAlphanumeric(code4)) && size++ < 63) {
      const next = code4 === 45 ? emailValue : emailLabel;
      effects.consume(code4);
      return next;
    }
    return nok(code4);
  }
}

// node_modules/micromark-core-commonmark/lib/blank-line.js
var blankLine = {
  partial: true,
  tokenize: tokenizeBlankLine
};
function tokenizeBlankLine(effects, ok3, nok) {
  return start2;
  function start2(code4) {
    return markdownSpace(code4) ? factorySpace(effects, after, "linePrefix")(code4) : after(code4);
  }
  function after(code4) {
    return code4 === null || markdownLineEnding(code4) ? ok3(code4) : nok(code4);
  }
}

// node_modules/micromark-core-commonmark/lib/block-quote.js
var blockQuote = {
  continuation: {
    tokenize: tokenizeBlockQuoteContinuation
  },
  exit,
  name: "blockQuote",
  tokenize: tokenizeBlockQuoteStart
};
function tokenizeBlockQuoteStart(effects, ok3, nok) {
  const self2 = this;
  return start2;
  function start2(code4) {
    if (code4 === 62) {
      const state2 = self2.containerState;
      if (!state2.open) {
        effects.enter("blockQuote", {
          _container: true
        });
        state2.open = true;
      }
      effects.enter("blockQuotePrefix");
      effects.enter("blockQuoteMarker");
      effects.consume(code4);
      effects.exit("blockQuoteMarker");
      return after;
    }
    return nok(code4);
  }
  function after(code4) {
    if (markdownSpace(code4)) {
      effects.enter("blockQuotePrefixWhitespace");
      effects.consume(code4);
      effects.exit("blockQuotePrefixWhitespace");
      effects.exit("blockQuotePrefix");
      return ok3;
    }
    effects.exit("blockQuotePrefix");
    return ok3(code4);
  }
}
function tokenizeBlockQuoteContinuation(effects, ok3, nok) {
  const self2 = this;
  return contStart;
  function contStart(code4) {
    if (markdownSpace(code4)) {
      return factorySpace(effects, contBefore, "linePrefix", self2.parser.constructs.disable.null.includes("codeIndented") ? void 0 : 4)(code4);
    }
    return contBefore(code4);
  }
  function contBefore(code4) {
    return effects.attempt(blockQuote, ok3, nok)(code4);
  }
}
function exit(effects) {
  effects.exit("blockQuote");
}

// node_modules/micromark-core-commonmark/lib/character-escape.js
var characterEscape = {
  name: "characterEscape",
  tokenize: tokenizeCharacterEscape
};
function tokenizeCharacterEscape(effects, ok3, nok) {
  return start2;
  function start2(code4) {
    effects.enter("characterEscape");
    effects.enter("escapeMarker");
    effects.consume(code4);
    effects.exit("escapeMarker");
    return inside;
  }
  function inside(code4) {
    if (asciiPunctuation(code4)) {
      effects.enter("characterEscapeValue");
      effects.consume(code4);
      effects.exit("characterEscapeValue");
      effects.exit("characterEscape");
      return ok3;
    }
    return nok(code4);
  }
}

// node_modules/micromark-core-commonmark/lib/character-reference.js
var characterReference = {
  name: "characterReference",
  tokenize: tokenizeCharacterReference
};
function tokenizeCharacterReference(effects, ok3, nok) {
  const self2 = this;
  let size = 0;
  let max;
  let test;
  return start2;
  function start2(code4) {
    effects.enter("characterReference");
    effects.enter("characterReferenceMarker");
    effects.consume(code4);
    effects.exit("characterReferenceMarker");
    return open;
  }
  function open(code4) {
    if (code4 === 35) {
      effects.enter("characterReferenceMarkerNumeric");
      effects.consume(code4);
      effects.exit("characterReferenceMarkerNumeric");
      return numeric;
    }
    effects.enter("characterReferenceValue");
    max = 31;
    test = asciiAlphanumeric;
    return value(code4);
  }
  function numeric(code4) {
    if (code4 === 88 || code4 === 120) {
      effects.enter("characterReferenceMarkerHexadecimal");
      effects.consume(code4);
      effects.exit("characterReferenceMarkerHexadecimal");
      effects.enter("characterReferenceValue");
      max = 6;
      test = asciiHexDigit;
      return value;
    }
    effects.enter("characterReferenceValue");
    max = 7;
    test = asciiDigit;
    return value(code4);
  }
  function value(code4) {
    if (code4 === 59 && size) {
      const token = effects.exit("characterReferenceValue");
      if (test === asciiAlphanumeric && !decodeNamedCharacterReference(self2.sliceSerialize(token))) {
        return nok(code4);
      }
      effects.enter("characterReferenceMarker");
      effects.consume(code4);
      effects.exit("characterReferenceMarker");
      effects.exit("characterReference");
      return ok3;
    }
    if (test(code4) && size++ < max) {
      effects.consume(code4);
      return value;
    }
    return nok(code4);
  }
}

// node_modules/micromark-core-commonmark/lib/partial-non-lazy-continuation.js
var nonLazyContinuation = {
  partial: true,
  tokenize: tokenizeNonLazyContinuation
};
function tokenizeNonLazyContinuation(effects, ok3, nok) {
  const self2 = this;
  return start2;
  function start2(code4) {
    if (code4 === null) {
      return nok(code4);
    }
    effects.enter("lineEnding");
    effects.consume(code4);
    effects.exit("lineEnding");
    return after;
  }
  function after(code4) {
    return self2.parser.lazy[self2.now().line] ? nok(code4) : ok3(code4);
  }
}

// node_modules/micromark-core-commonmark/lib/code-fenced.js
var codeFenced = {
  concrete: true,
  name: "codeFenced",
  tokenize: tokenizeCodeFenced
};
function tokenizeCodeFenced(effects, ok3, nok) {
  const self2 = this;
  const closeStart = {
    partial: true,
    tokenize: tokenizeCloseStart
  };
  let initialPrefix = 0;
  let sizeOpen = 0;
  let marker;
  return start2;
  function start2(code4) {
    return beforeSequenceOpen(code4);
  }
  function beforeSequenceOpen(code4) {
    const tail = self2.events[self2.events.length - 1];
    initialPrefix = tail && tail[1].type === "linePrefix" ? tail[2].sliceSerialize(tail[1], true).length : 0;
    marker = code4;
    effects.enter("codeFenced");
    effects.enter("codeFencedFence");
    effects.enter("codeFencedFenceSequence");
    return sequenceOpen(code4);
  }
  function sequenceOpen(code4) {
    if (code4 === marker) {
      sizeOpen++;
      effects.consume(code4);
      return sequenceOpen;
    }
    if (sizeOpen < 3) {
      return nok(code4);
    }
    effects.exit("codeFencedFenceSequence");
    return markdownSpace(code4) ? factorySpace(effects, infoBefore, "whitespace")(code4) : infoBefore(code4);
  }
  function infoBefore(code4) {
    if (code4 === null || markdownLineEnding(code4)) {
      effects.exit("codeFencedFence");
      return self2.interrupt ? ok3(code4) : effects.check(nonLazyContinuation, atNonLazyBreak, after)(code4);
    }
    effects.enter("codeFencedFenceInfo");
    effects.enter("chunkString", {
      contentType: "string"
    });
    return info(code4);
  }
  function info(code4) {
    if (code4 === null || markdownLineEnding(code4)) {
      effects.exit("chunkString");
      effects.exit("codeFencedFenceInfo");
      return infoBefore(code4);
    }
    if (markdownSpace(code4)) {
      effects.exit("chunkString");
      effects.exit("codeFencedFenceInfo");
      return factorySpace(effects, metaBefore, "whitespace")(code4);
    }
    if (code4 === 96 && code4 === marker) {
      return nok(code4);
    }
    effects.consume(code4);
    return info;
  }
  function metaBefore(code4) {
    if (code4 === null || markdownLineEnding(code4)) {
      return infoBefore(code4);
    }
    effects.enter("codeFencedFenceMeta");
    effects.enter("chunkString", {
      contentType: "string"
    });
    return meta(code4);
  }
  function meta(code4) {
    if (code4 === null || markdownLineEnding(code4)) {
      effects.exit("chunkString");
      effects.exit("codeFencedFenceMeta");
      return infoBefore(code4);
    }
    if (code4 === 96 && code4 === marker) {
      return nok(code4);
    }
    effects.consume(code4);
    return meta;
  }
  function atNonLazyBreak(code4) {
    return effects.attempt(closeStart, after, contentBefore)(code4);
  }
  function contentBefore(code4) {
    effects.enter("lineEnding");
    effects.consume(code4);
    effects.exit("lineEnding");
    return contentStart;
  }
  function contentStart(code4) {
    return initialPrefix > 0 && markdownSpace(code4) ? factorySpace(effects, beforeContentChunk, "linePrefix", initialPrefix + 1)(code4) : beforeContentChunk(code4);
  }
  function beforeContentChunk(code4) {
    if (code4 === null || markdownLineEnding(code4)) {
      return effects.check(nonLazyContinuation, atNonLazyBreak, after)(code4);
    }
    effects.enter("codeFlowValue");
    return contentChunk(code4);
  }
  function contentChunk(code4) {
    if (code4 === null || markdownLineEnding(code4)) {
      effects.exit("codeFlowValue");
      return beforeContentChunk(code4);
    }
    effects.consume(code4);
    return contentChunk;
  }
  function after(code4) {
    effects.exit("codeFenced");
    return ok3(code4);
  }
  function tokenizeCloseStart(effects2, ok4, nok2) {
    let size = 0;
    return startBefore;
    function startBefore(code4) {
      effects2.enter("lineEnding");
      effects2.consume(code4);
      effects2.exit("lineEnding");
      return start3;
    }
    function start3(code4) {
      effects2.enter("codeFencedFence");
      return markdownSpace(code4) ? factorySpace(effects2, beforeSequenceClose, "linePrefix", self2.parser.constructs.disable.null.includes("codeIndented") ? void 0 : 4)(code4) : beforeSequenceClose(code4);
    }
    function beforeSequenceClose(code4) {
      if (code4 === marker) {
        effects2.enter("codeFencedFenceSequence");
        return sequenceClose(code4);
      }
      return nok2(code4);
    }
    function sequenceClose(code4) {
      if (code4 === marker) {
        size++;
        effects2.consume(code4);
        return sequenceClose;
      }
      if (size >= sizeOpen) {
        effects2.exit("codeFencedFenceSequence");
        return markdownSpace(code4) ? factorySpace(effects2, sequenceCloseAfter, "whitespace")(code4) : sequenceCloseAfter(code4);
      }
      return nok2(code4);
    }
    function sequenceCloseAfter(code4) {
      if (code4 === null || markdownLineEnding(code4)) {
        effects2.exit("codeFencedFence");
        return ok4(code4);
      }
      return nok2(code4);
    }
  }
}

// node_modules/micromark-core-commonmark/lib/code-indented.js
var codeIndented = {
  name: "codeIndented",
  tokenize: tokenizeCodeIndented
};
var furtherStart = {
  partial: true,
  tokenize: tokenizeFurtherStart
};
function tokenizeCodeIndented(effects, ok3, nok) {
  return start2;
  function start2(code4) {
    effects.enter("codeIndented");
    return factorySpaceMinMax(effects, atBreak, nok, "linePrefix", 4, 4)(code4);
  }
  function atBreak(code4) {
    if (code4 === null) {
      return after(code4);
    }
    if (markdownLineEnding(code4)) {
      return effects.attempt(furtherStart, atBreak, after)(code4);
    }
    effects.enter("codeFlowValue");
    return inside(code4);
  }
  function inside(code4) {
    if (code4 === null || markdownLineEnding(code4)) {
      effects.exit("codeFlowValue");
      return atBreak(code4);
    }
    effects.consume(code4);
    return inside;
  }
  function after(code4) {
    effects.exit("codeIndented");
    return ok3(code4);
  }
}
function tokenizeFurtherStart(effects, ok3, nok) {
  const self2 = this;
  return furtherStart2;
  function furtherStart2(code4) {
    if (self2.parser.lazy[self2.now().line]) {
      return nok(code4);
    }
    if (markdownLineEnding(code4)) {
      effects.enter("lineEnding");
      effects.consume(code4);
      effects.exit("lineEnding");
      return furtherStart2;
    }
    return factorySpaceMinMax(effects, ok3, onNotEnoughPrefix, "linePrefix", 4, 4)(code4);
  }
  function onNotEnoughPrefix(code4) {
    return markdownLineEnding(code4) ? furtherStart2(code4) : nok(code4);
  }
}

// node_modules/micromark-core-commonmark/lib/code-text.js
var codeText = {
  name: "codeText",
  previous,
  resolve: resolveCodeText,
  tokenize: tokenizeCodeText
};
function resolveCodeText(events) {
  let tailExitIndex = events.length - 4;
  let headEnterIndex = 3;
  let index3;
  let enter;
  if ((events[headEnterIndex][1].type === "lineEnding" || events[headEnterIndex][1].type === "space") && (events[tailExitIndex][1].type === "lineEnding" || events[tailExitIndex][1].type === "space")) {
    index3 = headEnterIndex;
    while (++index3 < tailExitIndex) {
      if (events[index3][1].type === "codeTextData") {
        events[headEnterIndex][1].type = "codeTextPadding";
        events[tailExitIndex][1].type = "codeTextPadding";
        headEnterIndex += 2;
        tailExitIndex -= 2;
        break;
      }
    }
  }
  index3 = headEnterIndex - 1;
  tailExitIndex++;
  while (++index3 <= tailExitIndex) {
    if (enter === void 0) {
      if (index3 !== tailExitIndex && events[index3][1].type !== "lineEnding") {
        enter = index3;
      }
    } else if (index3 === tailExitIndex || events[index3][1].type === "lineEnding") {
      events[enter][1].type = "codeTextData";
      if (index3 !== enter + 2) {
        events[enter][1].end = events[index3 - 1][1].end;
        events.splice(enter + 2, index3 - enter - 2);
        tailExitIndex -= index3 - enter - 2;
        index3 = enter + 2;
      }
      enter = void 0;
    }
  }
  return events;
}
function previous(code4) {
  return code4 !== 96 || this.events[this.events.length - 1][1].type === "characterEscape";
}
function tokenizeCodeText(effects, ok3, nok) {
  const self2 = this;
  let sizeOpen = 0;
  let size;
  let token;
  return start2;
  function start2(code4) {
    effects.enter("codeText");
    effects.enter("codeTextSequence");
    return sequenceOpen(code4);
  }
  function sequenceOpen(code4) {
    if (code4 === 96) {
      effects.consume(code4);
      sizeOpen++;
      return sequenceOpen;
    }
    effects.exit("codeTextSequence");
    return between(code4);
  }
  function between(code4) {
    if (code4 === null) {
      return nok(code4);
    }
    if (code4 === 32) {
      effects.enter("space");
      effects.consume(code4);
      effects.exit("space");
      return between;
    }
    if (code4 === 96) {
      token = effects.enter("codeTextSequence");
      size = 0;
      return sequenceClose(code4);
    }
    if (markdownLineEnding(code4)) {
      effects.enter("lineEnding");
      effects.consume(code4);
      effects.exit("lineEnding");
      return between;
    }
    effects.enter("codeTextData");
    return data(code4);
  }
  function data(code4) {
    if (code4 === null || code4 === 32 || code4 === 96 || markdownLineEnding(code4)) {
      effects.exit("codeTextData");
      return between(code4);
    }
    effects.consume(code4);
    return data;
  }
  function sequenceClose(code4) {
    if (code4 === 96) {
      effects.consume(code4);
      size++;
      return sequenceClose;
    }
    if (size === sizeOpen) {
      effects.exit("codeTextSequence");
      effects.exit("codeText");
      return ok3(code4);
    }
    token.type = "codeTextData";
    return data(code4);
  }
}

// node_modules/micromark-util-subtokenize/lib/splice-buffer.js
var SpliceBuffer = class {
  /**
   * @param {ReadonlyArray<T> | null | undefined} [initial]
   *   Initial items (optional).
   * @returns
   *   Splice buffer.
   */
  constructor(initial) {
    this.left = initial ? [...initial] : [];
    this.right = [];
  }
  /**
   * Array access;
   * does not move the cursor.
   *
   * @param {number} index
   *   Index.
   * @return {T}
   *   Item.
   */
  get(index3) {
    if (index3 < 0 || index3 >= this.left.length + this.right.length) {
      throw new RangeError("Cannot access index `" + index3 + "` in a splice buffer of size `" + (this.left.length + this.right.length) + "`");
    }
    if (index3 < this.left.length) return this.left[index3];
    return this.right[this.right.length - index3 + this.left.length - 1];
  }
  /**
   * The length of the splice buffer, one greater than the largest index in the
   * array.
   */
  get length() {
    return this.left.length + this.right.length;
  }
  /**
   * Remove and return `list[0]`;
   * moves the cursor to `0`.
   *
   * @returns {T | undefined}
   *   Item, optional.
   */
  shift() {
    this.setCursor(0);
    return this.right.pop();
  }
  /**
   * Slice the buffer to get an array;
   * does not move the cursor.
   *
   * @param {number} start
   *   Start.
   * @param {number | null | undefined} [end]
   *   End (optional).
   * @returns {Array<T>}
   *   Array of items.
   */
  slice(start2, end) {
    const stop = end === null || end === void 0 ? Number.POSITIVE_INFINITY : end;
    if (stop < this.left.length) {
      return this.left.slice(start2, stop);
    }
    if (start2 > this.left.length) {
      return this.right.slice(this.right.length - stop + this.left.length, this.right.length - start2 + this.left.length).reverse();
    }
    return this.left.slice(start2).concat(this.right.slice(this.right.length - stop + this.left.length).reverse());
  }
  /**
   * Mimics the behavior of Array.prototype.splice() except for the change of
   * interface necessary to avoid segfaults when patching in very large arrays.
   *
   * This operation moves cursor is moved to `start` and results in the cursor
   * placed after any inserted items.
   *
   * @param {number} start
   *   Start;
   *   zero-based index at which to start changing the array;
   *   negative numbers count backwards from the end of the array and values
   *   that are out-of bounds are clamped to the appropriate end of the array.
   * @param {number | null | undefined} [deleteCount=0]
   *   Delete count (default: `0`);
   *   maximum number of elements to delete, starting from start.
   * @param {Array<T> | null | undefined} [items=[]]
   *   Items to include in place of the deleted items (default: `[]`).
   * @return {Array<T>}
   *   Any removed items.
   */
  splice(start2, deleteCount, items) {
    const count = deleteCount || 0;
    this.setCursor(Math.trunc(start2));
    const removed = this.right.splice(this.right.length - count, Number.POSITIVE_INFINITY);
    if (items) chunkedPush(this.left, items);
    return removed.reverse();
  }
  /**
   * Remove and return the highest-numbered item in the array, so
   * `list[list.length - 1]`;
   * Moves the cursor to `length`.
   *
   * @returns {T | undefined}
   *   Item, optional.
   */
  pop() {
    this.setCursor(Number.POSITIVE_INFINITY);
    return this.left.pop();
  }
  /**
   * Inserts a single item to the high-numbered side of the array;
   * moves the cursor to `length`.
   *
   * @param {T} item
   *   Item.
   * @returns {undefined}
   *   Nothing.
   */
  push(item) {
    this.setCursor(Number.POSITIVE_INFINITY);
    this.left.push(item);
  }
  /**
   * Inserts many items to the high-numbered side of the array.
   * Moves the cursor to `length`.
   *
   * @param {Array<T>} items
   *   Items.
   * @returns {undefined}
   *   Nothing.
   */
  pushMany(items) {
    this.setCursor(Number.POSITIVE_INFINITY);
    chunkedPush(this.left, items);
  }
  /**
   * Inserts a single item to the low-numbered side of the array;
   * Moves the cursor to `0`.
   *
   * @param {T} item
   *   Item.
   * @returns {undefined}
   *   Nothing.
   */
  unshift(item) {
    this.setCursor(0);
    this.right.push(item);
  }
  /**
   * Inserts many items to the low-numbered side of the array;
   * moves the cursor to `0`.
   *
   * @param {Array<T>} items
   *   Items.
   * @returns {undefined}
   *   Nothing.
   */
  unshiftMany(items) {
    this.setCursor(0);
    chunkedPush(this.right, items.reverse());
  }
  /**
   * Move the cursor to a specific position in the array. Requires
   * time proportional to the distance moved.
   *
   * If `n < 0`, the cursor will end up at the beginning.
   * If `n > length`, the cursor will end up at the end.
   *
   * @param {number} n
   *   Position.
   * @return {undefined}
   *   Nothing.
   */
  setCursor(n) {
    if (n === this.left.length || n > this.left.length && this.right.length === 0 || n < 0 && this.left.length === 0) return;
    if (n < this.left.length) {
      const removed = this.left.splice(n, Number.POSITIVE_INFINITY);
      chunkedPush(this.right, removed.reverse());
    } else {
      const removed = this.right.splice(this.left.length + this.right.length - n, Number.POSITIVE_INFINITY);
      chunkedPush(this.left, removed.reverse());
    }
  }
};
function chunkedPush(list4, right) {
  let chunkStart = 0;
  if (right.length < 1e4) {
    list4.push(...right);
  } else {
    while (chunkStart < right.length) {
      list4.push(...right.slice(chunkStart, chunkStart + 1e4));
      chunkStart += 1e4;
    }
  }
}

// node_modules/micromark-util-subtokenize/index.js
function subtokenize(eventsArray) {
  const jumps = {};
  let index3 = -1;
  let event;
  let lineIndex;
  let otherIndex;
  let otherEvent;
  let parameters;
  let subevents;
  let more;
  const events = new SpliceBuffer(eventsArray);
  while (++index3 < events.length) {
    while (index3 in jumps) {
      index3 = jumps[index3];
    }
    event = events.get(index3);
    if (index3 && event[1].type === "chunkFlow" && events.get(index3 - 1)[1].type === "listItemPrefix") {
      subevents = event[1]._tokenizer.events;
      otherIndex = 0;
      if (otherIndex < subevents.length && subevents[otherIndex][1].type === "lineEndingBlank") {
        otherIndex += 2;
      }
      if (otherIndex < subevents.length && subevents[otherIndex][1].type === "content") {
        while (++otherIndex < subevents.length) {
          if (subevents[otherIndex][1].type === "content") {
            break;
          }
          if (subevents[otherIndex][1].type === "chunkText") {
            subevents[otherIndex][1]._isInFirstContentOfListItem = true;
            otherIndex++;
          }
        }
      }
    }
    if (event[0] === "enter") {
      if (event[1].contentType) {
        Object.assign(jumps, subcontent(events, index3));
        index3 = jumps[index3];
        more = true;
      }
    } else if (event[1]._container) {
      otherIndex = index3;
      lineIndex = void 0;
      while (otherIndex--) {
        otherEvent = events.get(otherIndex);
        if (otherEvent[1].type === "lineEnding" || otherEvent[1].type === "lineEndingBlank") {
          if (otherEvent[0] === "enter") {
            if (lineIndex) {
              events.get(lineIndex)[1].type = "lineEndingBlank";
            }
            otherEvent[1].type = "lineEnding";
            lineIndex = otherIndex;
          }
        } else if (otherEvent[1].type === "linePrefix" || otherEvent[1].type === "listItemIndent") {
        } else {
          break;
        }
      }
      if (lineIndex) {
        event[1].end = {
          ...events.get(lineIndex)[1].start
        };
        parameters = events.slice(lineIndex, index3);
        parameters.unshift(event);
        events.splice(lineIndex, index3 - lineIndex + 1, parameters);
      }
    }
  }
  splice(eventsArray, 0, Number.POSITIVE_INFINITY, events.slice(0));
  return !more;
}
function subcontent(events, eventIndex) {
  const token = events.get(eventIndex)[1];
  const context = events.get(eventIndex)[2];
  let startPosition = eventIndex - 1;
  const startPositions = [];
  let tokenizer = token._tokenizer;
  if (!tokenizer) {
    tokenizer = context.parser[token.contentType](token.start);
    if (token._contentTypeTextTrailing) {
      tokenizer._contentTypeTextTrailing = true;
    }
  }
  const childEvents = tokenizer.events;
  const jumps = [];
  const gaps = {};
  let stream;
  let previous3;
  let index3 = -1;
  let current = token;
  let adjust = 0;
  let start2 = 0;
  const breaks = [start2];
  while (current) {
    while (events.get(++startPosition)[1] !== current) {
    }
    startPositions.push(startPosition);
    if (!current._tokenizer) {
      stream = context.sliceStream(current);
      if (!current.next) {
        stream.push(null);
      }
      if (previous3) {
        tokenizer.defineSkip(current.start);
      }
      if (current._isInFirstContentOfListItem) {
        tokenizer._gfmTasklistFirstContentOfListItem = true;
      }
      tokenizer.write(stream);
      if (current._isInFirstContentOfListItem) {
        tokenizer._gfmTasklistFirstContentOfListItem = void 0;
      }
    }
    previous3 = current;
    current = current.next;
  }
  current = token;
  while (++index3 < childEvents.length) {
    if (
      // Find a void token that includes a break.
      childEvents[index3][0] === "exit" && childEvents[index3 - 1][0] === "enter" && childEvents[index3][1].type === childEvents[index3 - 1][1].type && childEvents[index3][1].start.line !== childEvents[index3][1].end.line
    ) {
      start2 = index3 + 1;
      breaks.push(start2);
      current._tokenizer = void 0;
      current.previous = void 0;
      current = current.next;
    }
  }
  tokenizer.events = [];
  if (current) {
    current._tokenizer = void 0;
    current.previous = void 0;
  } else {
    breaks.pop();
  }
  index3 = breaks.length;
  while (index3--) {
    const slice = childEvents.slice(breaks[index3], breaks[index3 + 1]);
    const start3 = startPositions.pop();
    jumps.push([start3, start3 + slice.length - 1]);
    events.splice(start3, 2, slice);
  }
  jumps.reverse();
  index3 = -1;
  while (++index3 < jumps.length) {
    gaps[adjust + jumps[index3][0]] = adjust + jumps[index3][1];
    adjust += jumps[index3][1] - jumps[index3][0] - 1;
  }
  return gaps;
}

// node_modules/micromark-core-commonmark/lib/content.js
var content2 = {
  resolve: resolveContent,
  tokenize: tokenizeContent
};
var continuationConstruct = {
  partial: true,
  tokenize: tokenizeContinuation
};
function resolveContent(events) {
  subtokenize(events);
  return events;
}
function tokenizeContent(effects, ok3) {
  let previous3;
  return chunkStart;
  function chunkStart(code4) {
    effects.enter("content");
    previous3 = effects.enter("chunkContent", {
      contentType: "content"
    });
    return chunkInside(code4);
  }
  function chunkInside(code4) {
    if (code4 === null) {
      return contentEnd(code4);
    }
    if (markdownLineEnding(code4)) {
      return effects.check(continuationConstruct, contentContinue, contentEnd)(code4);
    }
    effects.consume(code4);
    return chunkInside;
  }
  function contentEnd(code4) {
    effects.exit("chunkContent");
    effects.exit("content");
    return ok3(code4);
  }
  function contentContinue(code4) {
    effects.consume(code4);
    effects.exit("chunkContent");
    previous3.next = effects.enter("chunkContent", {
      contentType: "content",
      previous: previous3
    });
    previous3 = previous3.next;
    return chunkInside;
  }
}
function tokenizeContinuation(effects, ok3, nok) {
  const self2 = this;
  return startLookahead;
  function startLookahead(code4) {
    effects.exit("chunkContent");
    effects.enter("lineEnding");
    effects.consume(code4);
    effects.exit("lineEnding");
    return factorySpace(effects, prefixed, "linePrefix");
  }
  function prefixed(code4) {
    if (code4 === null || markdownLineEnding(code4)) {
      return nok(code4);
    }
    const tail = self2.events[self2.events.length - 1];
    if (!self2.parser.constructs.disable.null.includes("codeIndented") && tail && tail[1].type === "linePrefix" && tail[2].sliceSerialize(tail[1], true).length >= 4) {
      return ok3(code4);
    }
    return effects.interrupt(self2.parser.constructs.flow, nok, ok3)(code4);
  }
}

// node_modules/micromark-factory-destination/index.js
function factoryDestination(effects, ok3, nok, type, literalType, literalMarkerType, rawType, stringType, max) {
  const limit = max || Number.POSITIVE_INFINITY;
  let balance = 0;
  return start2;
  function start2(code4) {
    if (code4 === 60) {
      effects.enter(type);
      effects.enter(literalType);
      effects.enter(literalMarkerType);
      effects.consume(code4);
      effects.exit(literalMarkerType);
      return enclosedBefore;
    }
    if (code4 === null || code4 === 32 || code4 === 41 || asciiControl(code4)) {
      return nok(code4);
    }
    effects.enter(type);
    effects.enter(rawType);
    effects.enter(stringType);
    effects.enter("chunkString", {
      contentType: "string"
    });
    return raw(code4);
  }
  function enclosedBefore(code4) {
    if (code4 === 62) {
      effects.enter(literalMarkerType);
      effects.consume(code4);
      effects.exit(literalMarkerType);
      effects.exit(literalType);
      effects.exit(type);
      return ok3;
    }
    effects.enter(stringType);
    effects.enter("chunkString", {
      contentType: "string"
    });
    return enclosed(code4);
  }
  function enclosed(code4) {
    if (code4 === 62) {
      effects.exit("chunkString");
      effects.exit(stringType);
      return enclosedBefore(code4);
    }
    if (code4 === null || code4 === 60 || markdownLineEnding(code4)) {
      return nok(code4);
    }
    effects.consume(code4);
    return code4 === 92 ? enclosedEscape : enclosed;
  }
  function enclosedEscape(code4) {
    if (code4 === 60 || code4 === 62 || code4 === 92) {
      effects.consume(code4);
      return enclosed;
    }
    return enclosed(code4);
  }
  function raw(code4) {
    if (!balance && (code4 === null || code4 === 41 || markdownLineEndingOrSpace(code4))) {
      effects.exit("chunkString");
      effects.exit(stringType);
      effects.exit(rawType);
      effects.exit(type);
      return ok3(code4);
    }
    if (balance < limit && code4 === 40) {
      effects.consume(code4);
      balance++;
      return raw;
    }
    if (code4 === 41) {
      effects.consume(code4);
      balance--;
      return raw;
    }
    if (code4 === null || code4 === 32 || code4 === 40 || asciiControl(code4)) {
      return nok(code4);
    }
    effects.consume(code4);
    return code4 === 92 ? rawEscape : raw;
  }
  function rawEscape(code4) {
    if (code4 === 40 || code4 === 41 || code4 === 92) {
      effects.consume(code4);
      return raw;
    }
    return raw(code4);
  }
}

// node_modules/micromark-factory-label/index.js
function factoryLabel(effects, ok3, nok, type, markerType, stringType) {
  const self2 = this;
  let size = 0;
  let seen;
  return start2;
  function start2(code4) {
    effects.enter(type);
    effects.enter(markerType);
    effects.consume(code4);
    effects.exit(markerType);
    effects.enter(stringType);
    return atBreak;
  }
  function atBreak(code4) {
    if (size > 999 || code4 === null || code4 === 91 || code4 === 93 && !seen || // To do: remove in the future once we’ve switched from
    // `micromark-extension-footnote` to `micromark-extension-gfm-footnote`,
    // which doesn’t need this.
    // Hidden footnotes hook.
    /* c8 ignore next 3 */
    code4 === 94 && !size && "_hiddenFootnoteSupport" in self2.parser.constructs) {
      return nok(code4);
    }
    if (code4 === 93) {
      effects.exit(stringType);
      effects.enter(markerType);
      effects.consume(code4);
      effects.exit(markerType);
      effects.exit(type);
      return ok3;
    }
    if (markdownLineEnding(code4)) {
      effects.enter("lineEnding");
      effects.consume(code4);
      effects.exit("lineEnding");
      return atBreak;
    }
    effects.enter("chunkString", {
      contentType: "string"
    });
    return labelInside(code4);
  }
  function labelInside(code4) {
    if (code4 === null || code4 === 91 || code4 === 93 || markdownLineEnding(code4) || size++ > 999) {
      effects.exit("chunkString");
      return atBreak(code4);
    }
    effects.consume(code4);
    if (!seen) seen = !markdownSpace(code4);
    return code4 === 92 ? labelEscape : labelInside;
  }
  function labelEscape(code4) {
    if (code4 === 91 || code4 === 92 || code4 === 93) {
      effects.consume(code4);
      size++;
      return labelInside;
    }
    return labelInside(code4);
  }
}

// node_modules/micromark-factory-title/index.js
function factoryTitle(effects, ok3, nok, type, markerType, stringType) {
  let marker;
  return start2;
  function start2(code4) {
    if (code4 === 34 || code4 === 39 || code4 === 40) {
      effects.enter(type);
      effects.enter(markerType);
      effects.consume(code4);
      effects.exit(markerType);
      marker = code4 === 40 ? 41 : code4;
      return begin;
    }
    return nok(code4);
  }
  function begin(code4) {
    if (code4 === marker) {
      effects.enter(markerType);
      effects.consume(code4);
      effects.exit(markerType);
      effects.exit(type);
      return ok3;
    }
    effects.enter(stringType);
    return atBreak(code4);
  }
  function atBreak(code4) {
    if (code4 === marker) {
      effects.exit(stringType);
      return begin(marker);
    }
    if (code4 === null) {
      return nok(code4);
    }
    if (markdownLineEnding(code4)) {
      effects.enter("lineEnding");
      effects.consume(code4);
      effects.exit("lineEnding");
      return factorySpace(effects, atBreak, "linePrefix");
    }
    effects.enter("chunkString", {
      contentType: "string"
    });
    return inside(code4);
  }
  function inside(code4) {
    if (code4 === marker || code4 === null || markdownLineEnding(code4)) {
      effects.exit("chunkString");
      return atBreak(code4);
    }
    effects.consume(code4);
    return code4 === 92 ? escape : inside;
  }
  function escape(code4) {
    if (code4 === marker || code4 === 92) {
      effects.consume(code4);
      return inside;
    }
    return inside(code4);
  }
}

// node_modules/micromark-factory-whitespace/index.js
function factoryWhitespace(effects, ok3) {
  let seen;
  return start2;
  function start2(code4) {
    if (markdownLineEnding(code4)) {
      effects.enter("lineEnding");
      effects.consume(code4);
      effects.exit("lineEnding");
      seen = true;
      return start2;
    }
    if (markdownSpace(code4)) {
      return factorySpace(effects, start2, seen ? "linePrefix" : "lineSuffix")(code4);
    }
    return ok3(code4);
  }
}

// node_modules/micromark-core-commonmark/lib/definition.js
var definition = {
  name: "definition",
  tokenize: tokenizeDefinition
};
var titleBefore = {
  partial: true,
  tokenize: tokenizeTitleBefore
};
function tokenizeDefinition(effects, ok3, nok) {
  const self2 = this;
  let identifier;
  return start2;
  function start2(code4) {
    effects.enter("definition");
    return before(code4);
  }
  function before(code4) {
    return factoryLabel.call(
      self2,
      effects,
      labelAfter,
      // Note: we don’t need to reset the way `markdown-rs` does.
      nok,
      "definitionLabel",
      "definitionLabelMarker",
      "definitionLabelString"
    )(code4);
  }
  function labelAfter(code4) {
    identifier = normalizeIdentifier(self2.sliceSerialize(self2.events[self2.events.length - 1][1]).slice(1, -1));
    if (code4 === 58) {
      effects.enter("definitionMarker");
      effects.consume(code4);
      effects.exit("definitionMarker");
      return markerAfter;
    }
    return nok(code4);
  }
  function markerAfter(code4) {
    return markdownLineEndingOrSpace(code4) ? factoryWhitespace(effects, destinationBefore)(code4) : destinationBefore(code4);
  }
  function destinationBefore(code4) {
    return factoryDestination(
      effects,
      destinationAfter,
      // Note: we don’t need to reset the way `markdown-rs` does.
      nok,
      "definitionDestination",
      "definitionDestinationLiteral",
      "definitionDestinationLiteralMarker",
      "definitionDestinationRaw",
      "definitionDestinationString"
    )(code4);
  }
  function destinationAfter(code4) {
    return effects.attempt(titleBefore, after, after)(code4);
  }
  function after(code4) {
    return markdownSpace(code4) ? factorySpace(effects, afterWhitespace, "whitespace")(code4) : afterWhitespace(code4);
  }
  function afterWhitespace(code4) {
    if (code4 === null || markdownLineEnding(code4)) {
      effects.exit("definition");
      self2.parser.defined.push(identifier);
      return ok3(code4);
    }
    return nok(code4);
  }
}
function tokenizeTitleBefore(effects, ok3, nok) {
  return titleBefore2;
  function titleBefore2(code4) {
    return markdownLineEndingOrSpace(code4) ? factoryWhitespace(effects, beforeMarker)(code4) : nok(code4);
  }
  function beforeMarker(code4) {
    return factoryTitle(effects, titleAfter, nok, "definitionTitle", "definitionTitleMarker", "definitionTitleString")(code4);
  }
  function titleAfter(code4) {
    return markdownSpace(code4) ? factorySpace(effects, titleAfterOptionalWhitespace, "whitespace")(code4) : titleAfterOptionalWhitespace(code4);
  }
  function titleAfterOptionalWhitespace(code4) {
    return code4 === null || markdownLineEnding(code4) ? ok3(code4) : nok(code4);
  }
}

// node_modules/micromark-core-commonmark/lib/hard-break-escape.js
var hardBreakEscape = {
  name: "hardBreakEscape",
  tokenize: tokenizeHardBreakEscape
};
function tokenizeHardBreakEscape(effects, ok3, nok) {
  return start2;
  function start2(code4) {
    effects.enter("hardBreakEscape");
    effects.consume(code4);
    return after;
  }
  function after(code4) {
    if (markdownLineEnding(code4)) {
      effects.exit("hardBreakEscape");
      return ok3(code4);
    }
    return nok(code4);
  }
}

// node_modules/micromark-core-commonmark/lib/heading-atx.js
var headingAtx = {
  name: "headingAtx",
  resolve: resolveHeadingAtx,
  tokenize: tokenizeHeadingAtx
};
function resolveHeadingAtx(events, context) {
  let contentEnd = events.length - 2;
  let contentStart = 3;
  if (events[contentStart][1].type === "whitespace") {
    contentStart += 2;
  }
  if (contentEnd - 2 > contentStart && events[contentEnd][1].type === "whitespace") {
    contentEnd -= 2;
  }
  if (events[contentEnd][1].type === "atxHeadingSequence" && (contentStart === contentEnd - 1 || contentEnd - 4 > contentStart && events[contentEnd - 2][1].type === "whitespace")) {
    contentEnd -= contentStart + 1 === contentEnd ? 2 : 4;
  }
  if (contentEnd > contentStart) {
    const content3 = {
      type: "atxHeadingText",
      start: events[contentStart][1].start,
      end: events[contentEnd][1].end
    };
    const text7 = {
      type: "chunkText",
      start: events[contentStart][1].start,
      end: events[contentEnd][1].end,
      contentType: "text"
    };
    splice(events, contentStart, contentEnd - contentStart + 1, [["enter", content3, context], ["enter", text7, context], ["exit", text7, context], ["exit", content3, context]]);
  }
  return events;
}
function tokenizeHeadingAtx(effects, ok3, nok) {
  let size = 0;
  return start2;
  function start2(code4) {
    effects.enter("atxHeading");
    return before(code4);
  }
  function before(code4) {
    effects.enter("atxHeadingSequence");
    return sequenceOpen(code4);
  }
  function sequenceOpen(code4) {
    if (code4 === 35 && size++ < 6) {
      effects.consume(code4);
      return sequenceOpen;
    }
    if (code4 === null || markdownLineEndingOrSpace(code4)) {
      effects.exit("atxHeadingSequence");
      return atBreak(code4);
    }
    return nok(code4);
  }
  function atBreak(code4) {
    if (code4 === 35) {
      effects.enter("atxHeadingSequence");
      return sequenceFurther(code4);
    }
    if (code4 === null || markdownLineEnding(code4)) {
      effects.exit("atxHeading");
      return ok3(code4);
    }
    if (markdownSpace(code4)) {
      return factorySpace(effects, atBreak, "whitespace")(code4);
    }
    effects.enter("atxHeadingText");
    return data(code4);
  }
  function sequenceFurther(code4) {
    if (code4 === 35) {
      effects.consume(code4);
      return sequenceFurther;
    }
    effects.exit("atxHeadingSequence");
    return atBreak(code4);
  }
  function data(code4) {
    if (code4 === null || code4 === 35 || markdownLineEndingOrSpace(code4)) {
      effects.exit("atxHeadingText");
      return atBreak(code4);
    }
    effects.consume(code4);
    return data;
  }
}

// node_modules/micromark-util-html-tag-name/index.js
var htmlBlockNames = [
  "address",
  "article",
  "aside",
  "base",
  "basefont",
  "blockquote",
  "body",
  "caption",
  "center",
  "col",
  "colgroup",
  "dd",
  "details",
  "dialog",
  "dir",
  "div",
  "dl",
  "dt",
  "fieldset",
  "figcaption",
  "figure",
  "footer",
  "form",
  "frame",
  "frameset",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "head",
  "header",
  "hr",
  "html",
  "iframe",
  "legend",
  "li",
  "link",
  "main",
  "menu",
  "menuitem",
  "nav",
  "noframes",
  "ol",
  "optgroup",
  "option",
  "p",
  "param",
  "search",
  "section",
  "summary",
  "table",
  "tbody",
  "td",
  "tfoot",
  "th",
  "thead",
  "title",
  "tr",
  "track",
  "ul"
];
var htmlRawNames = ["pre", "script", "style", "textarea"];

// node_modules/micromark-core-commonmark/lib/html-flow.js
var htmlFlow = {
  concrete: true,
  name: "htmlFlow",
  resolveTo: resolveToHtmlFlow,
  tokenize: tokenizeHtmlFlow
};
var blankLineBefore = {
  partial: true,
  tokenize: tokenizeBlankLineBefore
};
function resolveToHtmlFlow(events) {
  let index3 = events.length;
  while (index3--) {
    if (events[index3][0] === "enter" && events[index3][1].type === "htmlFlow") {
      break;
    }
  }
  if (index3 > 1 && events[index3 - 2][1].type === "linePrefix") {
    events[index3][1].start = events[index3 - 2][1].start;
    events[index3 + 1][1].start = events[index3 - 2][1].start;
    events.splice(index3 - 2, 2);
  }
  return events;
}
function tokenizeHtmlFlow(effects, ok3, nok) {
  const self2 = this;
  let marker;
  let closingTag;
  let buffer;
  let index3;
  let markerB;
  return start2;
  function start2(code4) {
    return before(code4);
  }
  function before(code4) {
    effects.enter("htmlFlow");
    effects.enter("htmlFlowData");
    effects.consume(code4);
    return open;
  }
  function open(code4) {
    if (code4 === 33) {
      effects.consume(code4);
      return declarationOpen;
    }
    if (code4 === 47) {
      effects.consume(code4);
      closingTag = true;
      return tagCloseStart;
    }
    if (code4 === 63) {
      effects.consume(code4);
      marker = 3;
      return self2.interrupt ? ok3 : continuationDeclarationInside;
    }
    if (asciiAlpha(code4)) {
      effects.consume(code4);
      buffer = String.fromCharCode(code4);
      return tagName;
    }
    return nok(code4);
  }
  function declarationOpen(code4) {
    if (code4 === 45) {
      effects.consume(code4);
      marker = 2;
      return commentOpenInside;
    }
    if (code4 === 91) {
      effects.consume(code4);
      marker = 5;
      index3 = 0;
      return cdataOpenInside;
    }
    if (asciiAlpha(code4)) {
      effects.consume(code4);
      marker = 4;
      return self2.interrupt ? ok3 : continuationDeclarationInside;
    }
    return nok(code4);
  }
  function commentOpenInside(code4) {
    if (code4 === 45) {
      effects.consume(code4);
      return self2.interrupt ? ok3 : continuationDeclarationInside;
    }
    return nok(code4);
  }
  function cdataOpenInside(code4) {
    const value = "CDATA[";
    if (code4 === value.charCodeAt(index3++)) {
      effects.consume(code4);
      if (index3 === value.length) {
        return self2.interrupt ? ok3 : continuation;
      }
      return cdataOpenInside;
    }
    return nok(code4);
  }
  function tagCloseStart(code4) {
    if (asciiAlpha(code4)) {
      effects.consume(code4);
      buffer = String.fromCharCode(code4);
      return tagName;
    }
    return nok(code4);
  }
  function tagName(code4) {
    if (code4 === null || code4 === 47 || code4 === 62 || markdownLineEndingOrSpace(code4)) {
      const slash = code4 === 47;
      const name2 = buffer.toLowerCase();
      if (!slash && !closingTag && htmlRawNames.includes(name2)) {
        marker = 1;
        return self2.interrupt ? ok3(code4) : continuation(code4);
      }
      if (htmlBlockNames.includes(buffer.toLowerCase())) {
        marker = 6;
        if (slash) {
          effects.consume(code4);
          return basicSelfClosing;
        }
        return self2.interrupt ? ok3(code4) : continuation(code4);
      }
      marker = 7;
      return self2.interrupt && !self2.parser.lazy[self2.now().line] ? nok(code4) : closingTag ? completeClosingTagAfter(code4) : completeAttributeNameBefore(code4);
    }
    if (code4 === 45 || asciiAlphanumeric(code4)) {
      effects.consume(code4);
      buffer += String.fromCharCode(code4);
      return tagName;
    }
    return nok(code4);
  }
  function basicSelfClosing(code4) {
    if (code4 === 62) {
      effects.consume(code4);
      return self2.interrupt ? ok3 : continuation;
    }
    return nok(code4);
  }
  function completeClosingTagAfter(code4) {
    if (markdownSpace(code4)) {
      effects.consume(code4);
      return completeClosingTagAfter;
    }
    return completeEnd(code4);
  }
  function completeAttributeNameBefore(code4) {
    if (code4 === 47) {
      effects.consume(code4);
      return completeEnd;
    }
    if (code4 === 58 || code4 === 95 || asciiAlpha(code4)) {
      effects.consume(code4);
      return completeAttributeName;
    }
    if (markdownSpace(code4)) {
      effects.consume(code4);
      return completeAttributeNameBefore;
    }
    return completeEnd(code4);
  }
  function completeAttributeName(code4) {
    if (code4 === 45 || code4 === 46 || code4 === 58 || code4 === 95 || asciiAlphanumeric(code4)) {
      effects.consume(code4);
      return completeAttributeName;
    }
    return completeAttributeNameAfter(code4);
  }
  function completeAttributeNameAfter(code4) {
    if (code4 === 61) {
      effects.consume(code4);
      return completeAttributeValueBefore;
    }
    if (markdownSpace(code4)) {
      effects.consume(code4);
      return completeAttributeNameAfter;
    }
    return completeAttributeNameBefore(code4);
  }
  function completeAttributeValueBefore(code4) {
    if (code4 === null || code4 === 60 || code4 === 61 || code4 === 62 || code4 === 96) {
      return nok(code4);
    }
    if (code4 === 34 || code4 === 39) {
      effects.consume(code4);
      markerB = code4;
      return completeAttributeValueQuoted;
    }
    if (markdownSpace(code4)) {
      effects.consume(code4);
      return completeAttributeValueBefore;
    }
    return completeAttributeValueUnquoted(code4);
  }
  function completeAttributeValueQuoted(code4) {
    if (code4 === markerB) {
      effects.consume(code4);
      markerB = null;
      return completeAttributeValueQuotedAfter;
    }
    if (code4 === null || markdownLineEnding(code4)) {
      return nok(code4);
    }
    effects.consume(code4);
    return completeAttributeValueQuoted;
  }
  function completeAttributeValueUnquoted(code4) {
    if (code4 === null || code4 === 34 || code4 === 39 || code4 === 47 || code4 === 60 || code4 === 61 || code4 === 62 || code4 === 96 || markdownLineEndingOrSpace(code4)) {
      return completeAttributeNameAfter(code4);
    }
    effects.consume(code4);
    return completeAttributeValueUnquoted;
  }
  function completeAttributeValueQuotedAfter(code4) {
    if (code4 === 47 || code4 === 62 || markdownSpace(code4)) {
      return completeAttributeNameBefore(code4);
    }
    return nok(code4);
  }
  function completeEnd(code4) {
    if (code4 === 62) {
      effects.consume(code4);
      return completeAfter;
    }
    return nok(code4);
  }
  function completeAfter(code4) {
    if (code4 === null || markdownLineEnding(code4)) {
      return continuation(code4);
    }
    if (markdownSpace(code4)) {
      effects.consume(code4);
      return completeAfter;
    }
    return nok(code4);
  }
  function continuation(code4) {
    if (code4 === 45 && marker === 2) {
      effects.consume(code4);
      return continuationCommentInside;
    }
    if (code4 === 60 && marker === 1) {
      effects.consume(code4);
      return continuationRawTagOpen;
    }
    if (code4 === 62 && marker === 4) {
      effects.consume(code4);
      return continuationClose;
    }
    if (code4 === 63 && marker === 3) {
      effects.consume(code4);
      return continuationDeclarationInside;
    }
    if (code4 === 93 && marker === 5) {
      effects.consume(code4);
      return continuationCdataInside;
    }
    if (markdownLineEnding(code4) && (marker === 6 || marker === 7)) {
      effects.exit("htmlFlowData");
      return effects.check(blankLineBefore, continuationAfter, continuationStart)(code4);
    }
    if (code4 === null || markdownLineEnding(code4)) {
      effects.exit("htmlFlowData");
      return continuationStart(code4);
    }
    effects.consume(code4);
    return continuation;
  }
  function continuationStart(code4) {
    return effects.check(nonLazyContinuation, continuationStartNonLazy, continuationAfter)(code4);
  }
  function continuationStartNonLazy(code4) {
    effects.enter("lineEnding");
    effects.consume(code4);
    effects.exit("lineEnding");
    return continuationBefore;
  }
  function continuationBefore(code4) {
    if (code4 === null || markdownLineEnding(code4)) {
      return continuationStart(code4);
    }
    effects.enter("htmlFlowData");
    return continuation(code4);
  }
  function continuationCommentInside(code4) {
    if (code4 === 45) {
      effects.consume(code4);
      return continuationDeclarationInside;
    }
    return continuation(code4);
  }
  function continuationRawTagOpen(code4) {
    if (code4 === 47) {
      effects.consume(code4);
      buffer = "";
      return continuationRawEndTag;
    }
    return continuation(code4);
  }
  function continuationRawEndTag(code4) {
    if (code4 === 62) {
      const name2 = buffer.toLowerCase();
      if (htmlRawNames.includes(name2)) {
        effects.consume(code4);
        return continuationClose;
      }
      return continuation(code4);
    }
    if (asciiAlpha(code4) && buffer.length < 8) {
      effects.consume(code4);
      buffer += String.fromCharCode(code4);
      return continuationRawEndTag;
    }
    return continuation(code4);
  }
  function continuationCdataInside(code4) {
    if (code4 === 93) {
      effects.consume(code4);
      return continuationDeclarationInside;
    }
    return continuation(code4);
  }
  function continuationDeclarationInside(code4) {
    if (code4 === 62) {
      effects.consume(code4);
      return continuationClose;
    }
    if (code4 === 45 && marker === 2) {
      effects.consume(code4);
      return continuationDeclarationInside;
    }
    return continuation(code4);
  }
  function continuationClose(code4) {
    if (code4 === null || markdownLineEnding(code4)) {
      effects.exit("htmlFlowData");
      return continuationAfter(code4);
    }
    effects.consume(code4);
    return continuationClose;
  }
  function continuationAfter(code4) {
    effects.exit("htmlFlow");
    return ok3(code4);
  }
}
function tokenizeBlankLineBefore(effects, ok3, nok) {
  return start2;
  function start2(code4) {
    effects.enter("lineEnding");
    effects.consume(code4);
    effects.exit("lineEnding");
    return effects.attempt(blankLine, ok3, nok);
  }
}

// node_modules/micromark-core-commonmark/lib/html-text.js
var htmlText = {
  name: "htmlText",
  tokenize: tokenizeHtmlText
};
function tokenizeHtmlText(effects, ok3, nok) {
  const self2 = this;
  let marker;
  let index3;
  let returnState;
  return start2;
  function start2(code4) {
    effects.enter("htmlText");
    effects.enter("htmlTextData");
    effects.consume(code4);
    return open;
  }
  function open(code4) {
    if (code4 === 33) {
      effects.consume(code4);
      return declarationOpen;
    }
    if (code4 === 47) {
      effects.consume(code4);
      return tagCloseStart;
    }
    if (code4 === 63) {
      effects.consume(code4);
      return instruction;
    }
    if (asciiAlpha(code4)) {
      effects.consume(code4);
      return tagOpen;
    }
    return nok(code4);
  }
  function declarationOpen(code4) {
    if (code4 === 45) {
      effects.consume(code4);
      return commentOpenInside;
    }
    if (code4 === 91) {
      effects.consume(code4);
      index3 = 0;
      return cdataOpenInside;
    }
    if (asciiAlpha(code4)) {
      effects.consume(code4);
      return declaration;
    }
    return nok(code4);
  }
  function commentOpenInside(code4) {
    if (code4 === 45) {
      effects.consume(code4);
      return commentEnd;
    }
    return nok(code4);
  }
  function comment(code4) {
    if (code4 === null) {
      return nok(code4);
    }
    if (code4 === 45) {
      effects.consume(code4);
      return commentClose;
    }
    if (markdownLineEnding(code4)) {
      returnState = comment;
      return lineEndingBefore(code4);
    }
    effects.consume(code4);
    return comment;
  }
  function commentClose(code4) {
    if (code4 === 45) {
      effects.consume(code4);
      return commentEnd;
    }
    return comment(code4);
  }
  function commentEnd(code4) {
    return code4 === 62 ? end(code4) : code4 === 45 ? commentClose(code4) : comment(code4);
  }
  function cdataOpenInside(code4) {
    const value = "CDATA[";
    if (code4 === value.charCodeAt(index3++)) {
      effects.consume(code4);
      return index3 === value.length ? cdata : cdataOpenInside;
    }
    return nok(code4);
  }
  function cdata(code4) {
    if (code4 === null) {
      return nok(code4);
    }
    if (code4 === 93) {
      effects.consume(code4);
      return cdataClose;
    }
    if (markdownLineEnding(code4)) {
      returnState = cdata;
      return lineEndingBefore(code4);
    }
    effects.consume(code4);
    return cdata;
  }
  function cdataClose(code4) {
    if (code4 === 93) {
      effects.consume(code4);
      return cdataEnd;
    }
    return cdata(code4);
  }
  function cdataEnd(code4) {
    if (code4 === 62) {
      return end(code4);
    }
    if (code4 === 93) {
      effects.consume(code4);
      return cdataEnd;
    }
    return cdata(code4);
  }
  function declaration(code4) {
    if (code4 === null || code4 === 62) {
      return end(code4);
    }
    if (markdownLineEnding(code4)) {
      returnState = declaration;
      return lineEndingBefore(code4);
    }
    effects.consume(code4);
    return declaration;
  }
  function instruction(code4) {
    if (code4 === null) {
      return nok(code4);
    }
    if (code4 === 63) {
      effects.consume(code4);
      return instructionClose;
    }
    if (markdownLineEnding(code4)) {
      returnState = instruction;
      return lineEndingBefore(code4);
    }
    effects.consume(code4);
    return instruction;
  }
  function instructionClose(code4) {
    return code4 === 62 ? end(code4) : instruction(code4);
  }
  function tagCloseStart(code4) {
    if (asciiAlpha(code4)) {
      effects.consume(code4);
      return tagClose;
    }
    return nok(code4);
  }
  function tagClose(code4) {
    if (code4 === 45 || asciiAlphanumeric(code4)) {
      effects.consume(code4);
      return tagClose;
    }
    return tagCloseBetween(code4);
  }
  function tagCloseBetween(code4) {
    if (markdownLineEnding(code4)) {
      returnState = tagCloseBetween;
      return lineEndingBefore(code4);
    }
    if (markdownSpace(code4)) {
      effects.consume(code4);
      return tagCloseBetween;
    }
    return end(code4);
  }
  function tagOpen(code4) {
    if (code4 === 45 || asciiAlphanumeric(code4)) {
      effects.consume(code4);
      return tagOpen;
    }
    if (code4 === 47 || code4 === 62 || markdownLineEndingOrSpace(code4)) {
      return tagOpenBetween(code4);
    }
    return nok(code4);
  }
  function tagOpenBetween(code4) {
    if (code4 === 47) {
      effects.consume(code4);
      return end;
    }
    if (code4 === 58 || code4 === 95 || asciiAlpha(code4)) {
      effects.consume(code4);
      return tagOpenAttributeName;
    }
    if (markdownLineEnding(code4)) {
      returnState = tagOpenBetween;
      return lineEndingBefore(code4);
    }
    if (markdownSpace(code4)) {
      effects.consume(code4);
      return tagOpenBetween;
    }
    return end(code4);
  }
  function tagOpenAttributeName(code4) {
    if (code4 === 45 || code4 === 46 || code4 === 58 || code4 === 95 || asciiAlphanumeric(code4)) {
      effects.consume(code4);
      return tagOpenAttributeName;
    }
    return tagOpenAttributeNameAfter(code4);
  }
  function tagOpenAttributeNameAfter(code4) {
    if (code4 === 61) {
      effects.consume(code4);
      return tagOpenAttributeValueBefore;
    }
    if (markdownLineEnding(code4)) {
      returnState = tagOpenAttributeNameAfter;
      return lineEndingBefore(code4);
    }
    if (markdownSpace(code4)) {
      effects.consume(code4);
      return tagOpenAttributeNameAfter;
    }
    return tagOpenBetween(code4);
  }
  function tagOpenAttributeValueBefore(code4) {
    if (code4 === null || code4 === 60 || code4 === 61 || code4 === 62 || code4 === 96) {
      return nok(code4);
    }
    if (code4 === 34 || code4 === 39) {
      effects.consume(code4);
      marker = code4;
      return tagOpenAttributeValueQuoted;
    }
    if (markdownLineEnding(code4)) {
      returnState = tagOpenAttributeValueBefore;
      return lineEndingBefore(code4);
    }
    if (markdownSpace(code4)) {
      effects.consume(code4);
      return tagOpenAttributeValueBefore;
    }
    effects.consume(code4);
    return tagOpenAttributeValueUnquoted;
  }
  function tagOpenAttributeValueQuoted(code4) {
    if (code4 === marker) {
      effects.consume(code4);
      marker = void 0;
      return tagOpenAttributeValueQuotedAfter;
    }
    if (code4 === null) {
      return nok(code4);
    }
    if (markdownLineEnding(code4)) {
      returnState = tagOpenAttributeValueQuoted;
      return lineEndingBefore(code4);
    }
    effects.consume(code4);
    return tagOpenAttributeValueQuoted;
  }
  function tagOpenAttributeValueUnquoted(code4) {
    if (code4 === null || code4 === 34 || code4 === 39 || code4 === 60 || code4 === 61 || code4 === 96) {
      return nok(code4);
    }
    if (code4 === 47 || code4 === 62 || markdownLineEndingOrSpace(code4)) {
      return tagOpenBetween(code4);
    }
    effects.consume(code4);
    return tagOpenAttributeValueUnquoted;
  }
  function tagOpenAttributeValueQuotedAfter(code4) {
    if (code4 === 47 || code4 === 62 || markdownLineEndingOrSpace(code4)) {
      return tagOpenBetween(code4);
    }
    return nok(code4);
  }
  function end(code4) {
    if (code4 === 62) {
      effects.consume(code4);
      effects.exit("htmlTextData");
      effects.exit("htmlText");
      return ok3;
    }
    return nok(code4);
  }
  function lineEndingBefore(code4) {
    effects.exit("htmlTextData");
    effects.enter("lineEnding");
    effects.consume(code4);
    effects.exit("lineEnding");
    return lineEndingAfter;
  }
  function lineEndingAfter(code4) {
    return markdownSpace(code4) ? factorySpace(effects, lineEndingAfterPrefix, "linePrefix", self2.parser.constructs.disable.null.includes("codeIndented") ? void 0 : 4)(code4) : lineEndingAfterPrefix(code4);
  }
  function lineEndingAfterPrefix(code4) {
    effects.enter("htmlTextData");
    return returnState(code4);
  }
}

// node_modules/micromark-core-commonmark/lib/label-end.js
var labelEnd = {
  name: "labelEnd",
  resolveAll: resolveAllLabelEnd,
  resolveTo: resolveToLabelEnd,
  tokenize: tokenizeLabelEnd
};
var resourceConstruct = {
  tokenize: tokenizeResource
};
var referenceFullConstruct = {
  tokenize: tokenizeReferenceFull
};
var referenceCollapsedConstruct = {
  tokenize: tokenizeReferenceCollapsed
};
function resolveAllLabelEnd(events) {
  let index3 = -1;
  const newEvents = [];
  while (++index3 < events.length) {
    const token = events[index3][1];
    newEvents.push(events[index3]);
    if (token.type === "labelImage" || token.type === "labelLink" || token.type === "labelEnd") {
      const offset = token.type === "labelImage" ? 4 : 2;
      token.type = "data";
      index3 += offset;
    }
  }
  if (events.length !== newEvents.length) {
    splice(events, 0, events.length, newEvents);
  }
  return events;
}
function resolveToLabelEnd(events, context) {
  let index3 = events.length;
  let offset = 0;
  let open;
  let close;
  let media;
  while (index3--) {
    const token = events[index3][1];
    if (open) {
      if (token.type === "link" || token.type === "labelLink" && token._inactive) {
        break;
      }
      if (events[index3][0] === "enter" && token.type === "labelLink") {
        token._inactive = true;
      }
    } else if (close) {
      if (events[index3][0] === "enter" && (token.type === "labelImage" || token.type === "labelLink") && !token._balanced) {
        open = index3;
        if (token.type !== "labelLink") {
          offset = 2;
          break;
        }
      }
    } else if (token.type === "labelEnd") {
      close = index3;
    }
  }
  const group = {
    type: events[open][1].type === "labelLink" ? "link" : "image",
    start: {
      ...events[open][1].start
    },
    end: {
      ...events[events.length - 1][1].end
    }
  };
  const label = {
    type: "label",
    start: {
      ...events[open][1].start
    },
    end: {
      ...events[close][1].end
    }
  };
  const text7 = {
    type: "labelText",
    start: {
      ...events[open + offset + 2][1].end
    },
    end: {
      ...events[close - 2][1].start
    }
  };
  media = [["enter", group, context], ["enter", label, context]];
  media = push(media, events.slice(open + 1, open + offset + 3));
  media = push(media, [["enter", text7, context]]);
  media = push(media, resolveAll(context.parser.constructs.insideSpan.null, events.slice(open + offset + 4, close - 3), context));
  media = push(media, [["exit", text7, context], events[close - 2], events[close - 1], ["exit", label, context]]);
  media = push(media, events.slice(close + 1));
  media = push(media, [["exit", group, context]]);
  splice(events, open, events.length, media);
  return events;
}
function tokenizeLabelEnd(effects, ok3, nok) {
  const self2 = this;
  const labelStarts = self2._labelStarts;
  let labelStart;
  let defined;
  if (labelStarts) {
    while (labelStarts.length > 0 && labelStarts[labelStarts.length - 1]._balanced) {
      labelStarts.pop();
    }
    labelStart = labelStarts[labelStarts.length - 1];
  }
  return start2;
  function start2(code4) {
    if (!labelStart) {
      return nok(code4);
    }
    if (labelStart._inactive) {
      return labelEndNok(code4);
    }
    defined = self2.parser.defined.includes(normalizeIdentifier(self2.sliceSerialize({
      start: labelStart.end,
      end: self2.now()
    })));
    effects.enter("labelEnd");
    effects.enter("labelMarker");
    effects.consume(code4);
    effects.exit("labelMarker");
    effects.exit("labelEnd");
    return after;
  }
  function after(code4) {
    if (code4 === 40) {
      return effects.attempt(resourceConstruct, labelEndOk, defined ? labelEndOk : labelEndNok)(code4);
    }
    if (code4 === 91) {
      return effects.attempt(referenceFullConstruct, labelEndOk, defined ? referenceNotFull : labelEndNok)(code4);
    }
    return defined ? labelEndOk(code4) : labelEndNok(code4);
  }
  function referenceNotFull(code4) {
    return effects.attempt(referenceCollapsedConstruct, labelEndOk, labelEndNok)(code4);
  }
  function labelEndOk(code4) {
    labelStarts.pop();
    return ok3(code4);
  }
  function labelEndNok(code4) {
    labelStart._balanced = true;
    return nok(code4);
  }
}
function tokenizeResource(effects, ok3, nok) {
  return resourceStart;
  function resourceStart(code4) {
    effects.enter("resource");
    effects.enter("resourceMarker");
    effects.consume(code4);
    effects.exit("resourceMarker");
    return resourceBefore;
  }
  function resourceBefore(code4) {
    return markdownLineEndingOrSpace(code4) ? factoryWhitespace(effects, resourceOpen)(code4) : resourceOpen(code4);
  }
  function resourceOpen(code4) {
    if (code4 === 41) {
      return resourceEnd(code4);
    }
    return factoryDestination(effects, resourceDestinationAfter, resourceDestinationMissing, "resourceDestination", "resourceDestinationLiteral", "resourceDestinationLiteralMarker", "resourceDestinationRaw", "resourceDestinationString", 32)(code4);
  }
  function resourceDestinationAfter(code4) {
    return markdownLineEndingOrSpace(code4) ? factoryWhitespace(effects, resourceBetween)(code4) : resourceEnd(code4);
  }
  function resourceDestinationMissing(code4) {
    return nok(code4);
  }
  function resourceBetween(code4) {
    if (code4 === 34 || code4 === 39 || code4 === 40) {
      return factoryTitle(effects, resourceTitleAfter, nok, "resourceTitle", "resourceTitleMarker", "resourceTitleString")(code4);
    }
    return resourceEnd(code4);
  }
  function resourceTitleAfter(code4) {
    return markdownLineEndingOrSpace(code4) ? factoryWhitespace(effects, resourceEnd)(code4) : resourceEnd(code4);
  }
  function resourceEnd(code4) {
    if (code4 === 41) {
      effects.enter("resourceMarker");
      effects.consume(code4);
      effects.exit("resourceMarker");
      effects.exit("resource");
      return ok3;
    }
    return nok(code4);
  }
}
function tokenizeReferenceFull(effects, ok3, nok) {
  const self2 = this;
  return referenceFull;
  function referenceFull(code4) {
    return factoryLabel.call(self2, effects, referenceFullAfter, referenceFullMissing, "reference", "referenceMarker", "referenceString")(code4);
  }
  function referenceFullAfter(code4) {
    return self2.parser.defined.includes(normalizeIdentifier(self2.sliceSerialize(self2.events[self2.events.length - 1][1]).slice(1, -1))) ? ok3(code4) : nok(code4);
  }
  function referenceFullMissing(code4) {
    return nok(code4);
  }
}
function tokenizeReferenceCollapsed(effects, ok3, nok) {
  return referenceCollapsedStart;
  function referenceCollapsedStart(code4) {
    effects.enter("reference");
    effects.enter("referenceMarker");
    effects.consume(code4);
    effects.exit("referenceMarker");
    return referenceCollapsedOpen;
  }
  function referenceCollapsedOpen(code4) {
    if (code4 === 93) {
      effects.enter("referenceMarker");
      effects.consume(code4);
      effects.exit("referenceMarker");
      effects.exit("reference");
      return ok3;
    }
    return nok(code4);
  }
}

// node_modules/micromark-core-commonmark/lib/label-start-image.js
var labelStartImage = {
  name: "labelStartImage",
  resolveAll: labelEnd.resolveAll,
  tokenize: tokenizeLabelStartImage
};
function tokenizeLabelStartImage(effects, ok3, nok) {
  const self2 = this;
  let labelImage;
  return start2;
  function start2(code4) {
    effects.enter("labelImage");
    effects.enter("labelImageMarker");
    effects.consume(code4);
    effects.exit("labelImageMarker");
    return open;
  }
  function open(code4) {
    if (code4 === 91) {
      effects.enter("labelMarker");
      effects.consume(code4);
      effects.exit("labelMarker");
      labelImage = effects.exit("labelImage");
      return after;
    }
    return nok(code4);
  }
  function after(code4) {
    if (code4 === 94 && "_hiddenFootnoteSupport" in self2.parser.constructs) {
      return nok(code4);
    }
    self2._labelStarts = self2._labelStarts || [];
    self2._labelStarts.push(labelImage);
    return ok3(code4);
  }
}

// node_modules/micromark-core-commonmark/lib/label-start-link.js
var labelStartLink = {
  name: "labelStartLink",
  resolveAll: labelEnd.resolveAll,
  tokenize: tokenizeLabelStartLink
};
function tokenizeLabelStartLink(effects, ok3, nok) {
  const self2 = this;
  let labelLink;
  return start2;
  function start2(code4) {
    effects.enter("labelLink");
    effects.enter("labelMarker");
    effects.consume(code4);
    effects.exit("labelMarker");
    labelLink = effects.exit("labelLink");
    return after;
  }
  function after(code4) {
    if (code4 === 94 && "_hiddenFootnoteSupport" in self2.parser.constructs) {
      return nok(code4);
    }
    self2._labelStarts = self2._labelStarts || [];
    self2._labelStarts.push(labelLink);
    return ok3(code4);
  }
}

// node_modules/micromark-core-commonmark/lib/line-ending.js
var lineEnding = {
  name: "lineEnding",
  tokenize: tokenizeLineEnding
};
function tokenizeLineEnding(effects, ok3) {
  return start2;
  function start2(code4) {
    effects.enter("lineEnding");
    effects.consume(code4);
    effects.exit("lineEnding");
    return factorySpace(effects, ok3, "linePrefix");
  }
}

// node_modules/micromark-core-commonmark/lib/thematic-break.js
var thematicBreak = {
  name: "thematicBreak",
  tokenize: tokenizeThematicBreak
};
function tokenizeThematicBreak(effects, ok3, nok) {
  let size = 0;
  let marker;
  return start2;
  function start2(code4) {
    effects.enter("thematicBreak");
    return before(code4);
  }
  function before(code4) {
    marker = code4;
    return atBreak(code4);
  }
  function atBreak(code4) {
    if (code4 === marker) {
      effects.enter("thematicBreakSequence");
      return sequence(code4);
    }
    if (size >= 3 && (code4 === null || markdownLineEnding(code4))) {
      effects.exit("thematicBreak");
      return ok3(code4);
    }
    return nok(code4);
  }
  function sequence(code4) {
    if (code4 === marker) {
      effects.consume(code4);
      size++;
      return sequence;
    }
    effects.exit("thematicBreakSequence");
    return markdownSpace(code4) ? factorySpace(effects, atBreak, "whitespace")(code4) : atBreak(code4);
  }
}

// node_modules/micromark-core-commonmark/lib/list.js
var list = {
  continuation: {
    tokenize: tokenizeListContinuation
  },
  exit: tokenizeListEnd,
  name: "list",
  tokenize: tokenizeListStart
};
var listItemPrefixWhitespaceConstruct = {
  partial: true,
  tokenize: tokenizeListItemPrefixWhitespace
};
var indentConstruct = {
  partial: true,
  tokenize: tokenizeIndent
};
function tokenizeListStart(effects, ok3, nok) {
  const self2 = this;
  const tail = self2.events[self2.events.length - 1];
  let initialSize = tail && tail[1].type === "linePrefix" ? tail[2].sliceSerialize(tail[1], true).length : 0;
  let size = 0;
  return start2;
  function start2(code4) {
    const kind = self2.containerState.type || (code4 === 42 || code4 === 43 || code4 === 45 ? "listUnordered" : "listOrdered");
    if (kind === "listUnordered" ? !self2.containerState.marker || code4 === self2.containerState.marker : asciiDigit(code4)) {
      if (!self2.containerState.type) {
        self2.containerState.type = kind;
        effects.enter(kind, {
          _container: true
        });
      }
      if (kind === "listUnordered") {
        effects.enter("listItemPrefix");
        return code4 === 42 || code4 === 45 ? effects.check(thematicBreak, nok, atMarker)(code4) : atMarker(code4);
      }
      if (!self2.interrupt || code4 === 49) {
        effects.enter("listItemPrefix");
        effects.enter("listItemValue");
        return inside(code4);
      }
    }
    return nok(code4);
  }
  function inside(code4) {
    if (asciiDigit(code4) && ++size < 10) {
      effects.consume(code4);
      return inside;
    }
    if ((!self2.interrupt || size < 2) && (self2.containerState.marker ? code4 === self2.containerState.marker : code4 === 41 || code4 === 46)) {
      effects.exit("listItemValue");
      return atMarker(code4);
    }
    return nok(code4);
  }
  function atMarker(code4) {
    effects.enter("listItemMarker");
    effects.consume(code4);
    effects.exit("listItemMarker");
    self2.containerState.marker = self2.containerState.marker || code4;
    return effects.check(
      blankLine,
      // Can’t be empty when interrupting.
      self2.interrupt ? nok : onBlank,
      effects.attempt(listItemPrefixWhitespaceConstruct, endOfPrefix, otherPrefix)
    );
  }
  function onBlank(code4) {
    self2.containerState.initialBlankLine = true;
    initialSize++;
    return endOfPrefix(code4);
  }
  function otherPrefix(code4) {
    if (markdownSpace(code4)) {
      effects.enter("listItemPrefixWhitespace");
      effects.consume(code4);
      effects.exit("listItemPrefixWhitespace");
      return endOfPrefix;
    }
    return nok(code4);
  }
  function endOfPrefix(code4) {
    self2.containerState.size = initialSize + self2.sliceSerialize(effects.exit("listItemPrefix"), true).length;
    return ok3(code4);
  }
}
function tokenizeListContinuation(effects, ok3, nok) {
  const self2 = this;
  self2.containerState._closeFlow = void 0;
  return effects.check(blankLine, onBlank, notBlank);
  function onBlank(code4) {
    self2.containerState.furtherBlankLines = self2.containerState.furtherBlankLines || self2.containerState.initialBlankLine;
    return factorySpace(effects, ok3, "listItemIndent", self2.containerState.size + 1)(code4);
  }
  function notBlank(code4) {
    if (self2.containerState.furtherBlankLines || !markdownSpace(code4)) {
      self2.containerState.furtherBlankLines = void 0;
      self2.containerState.initialBlankLine = void 0;
      return notInCurrentItem(code4);
    }
    self2.containerState.furtherBlankLines = void 0;
    self2.containerState.initialBlankLine = void 0;
    return effects.attempt(indentConstruct, ok3, notInCurrentItem)(code4);
  }
  function notInCurrentItem(code4) {
    self2.containerState._closeFlow = true;
    self2.interrupt = void 0;
    return factorySpace(effects, effects.attempt(list, ok3, nok), "linePrefix", self2.parser.constructs.disable.null.includes("codeIndented") ? void 0 : 4)(code4);
  }
}
function tokenizeIndent(effects, ok3, nok) {
  const self2 = this;
  return factorySpace(effects, afterPrefix, "listItemIndent", self2.containerState.size + 1);
  function afterPrefix(code4) {
    const tail = self2.events[self2.events.length - 1];
    return tail && tail[1].type === "listItemIndent" && tail[2].sliceSerialize(tail[1], true).length === self2.containerState.size ? ok3(code4) : nok(code4);
  }
}
function tokenizeListEnd(effects) {
  effects.exit(this.containerState.type);
}
function tokenizeListItemPrefixWhitespace(effects, ok3, nok) {
  const self2 = this;
  return factorySpace(effects, afterPrefix, "listItemPrefixWhitespace", self2.parser.constructs.disable.null.includes("codeIndented") ? void 0 : 4 + 1);
  function afterPrefix(code4) {
    const tail = self2.events[self2.events.length - 1];
    return !markdownSpace(code4) && tail && tail[1].type === "listItemPrefixWhitespace" ? ok3(code4) : nok(code4);
  }
}

// node_modules/micromark-core-commonmark/lib/setext-underline.js
var setextUnderline = {
  name: "setextUnderline",
  resolveTo: resolveToSetextUnderline,
  tokenize: tokenizeSetextUnderline
};
function resolveToSetextUnderline(events, context) {
  const editMap = new EditMap();
  let index3 = events.length;
  let content3;
  let text7;
  let definition3;
  while (index3--) {
    if (events[index3][0] === "enter") {
      if (events[index3][1].type === "content") {
        content3 = index3;
        break;
      }
      if (events[index3][1].type === "paragraph") {
        text7 = index3;
      }
    } else {
      if (events[index3][1].type === "content") {
        editMap.add(index3, 1, []);
      }
      if (!definition3 && events[index3][1].type === "definition") {
        definition3 = index3;
      }
    }
  }
  const heading3 = {
    type: "setextHeading",
    start: {
      ...events[content3][1].start
    },
    end: {
      ...events[events.length - 1][1].end
    }
  };
  events[text7][1].type = "setextHeadingText";
  if (definition3) {
    editMap.add(text7, 0, [["enter", heading3, context]]);
    editMap.add(definition3 + 1, 0, [["exit", events[content3][1], context]]);
    events[content3][1].end = {
      ...events[definition3][1].end
    };
  } else {
    events[content3][1] = heading3;
  }
  editMap.add(events.length, 0, [["exit", heading3, context]]);
  editMap.consume(events);
  return events;
}
function tokenizeSetextUnderline(effects, ok3, nok) {
  const self2 = this;
  let marker;
  return start2;
  function start2(code4) {
    let index3 = self2.events.length;
    let paragraph3;
    while (index3--) {
      if (self2.events[index3][1].type !== "lineEnding" && self2.events[index3][1].type !== "linePrefix" && self2.events[index3][1].type !== "content") {
        paragraph3 = self2.events[index3][1].type === "paragraph";
        break;
      }
    }
    if (!self2.parser.lazy[self2.now().line] && (self2.interrupt || paragraph3)) {
      effects.enter("setextHeadingLine");
      marker = code4;
      return before(code4);
    }
    return nok(code4);
  }
  function before(code4) {
    effects.enter("setextHeadingLineSequence");
    return inside(code4);
  }
  function inside(code4) {
    if (code4 === marker) {
      effects.consume(code4);
      return inside;
    }
    effects.exit("setextHeadingLineSequence");
    return markdownSpace(code4) ? factorySpace(effects, after, "lineSuffix")(code4) : after(code4);
  }
  function after(code4) {
    if (code4 === null || markdownLineEnding(code4)) {
      effects.exit("setextHeadingLine");
      return ok3(code4);
    }
    return nok(code4);
  }
}

// node_modules/micromark/lib/initialize/flow.js
var flow = {
  tokenize: initializeFlow
};
function initializeFlow(effects) {
  const self2 = this;
  const initial = effects.attempt(
    // Try to parse a blank line.
    blankLine,
    atBlankEnding,
    // Try to parse initial flow (essentially, only code).
    effects.attempt(this.parser.constructs.flowInitial, afterConstruct, factorySpace(effects, effects.attempt(this.parser.constructs.flow, afterConstruct, effects.attempt(content2, afterConstruct)), "linePrefix"))
  );
  return initial;
  function atBlankEnding(code4) {
    if (code4 === null) {
      effects.consume(code4);
      return;
    }
    effects.enter("lineEndingBlank");
    effects.consume(code4);
    effects.exit("lineEndingBlank");
    self2.currentConstruct = void 0;
    return initial;
  }
  function afterConstruct(code4) {
    if (code4 === null) {
      effects.consume(code4);
      return;
    }
    effects.enter("lineEnding");
    effects.consume(code4);
    effects.exit("lineEnding");
    self2.currentConstruct = void 0;
    return initial;
  }
}

// node_modules/micromark/lib/initialize/text.js
var resolver = {
  resolveAll: createResolver()
};
var string = initializeFactory("string");
var text2 = initializeFactory("text");
function initializeFactory(field) {
  return {
    resolveAll: createResolver(field === "text" ? resolveAllLineSuffixes : void 0),
    tokenize: initializeText
  };
  function initializeText(effects) {
    const self2 = this;
    const constructs2 = this.parser.constructs[field];
    const text7 = effects.attempt(constructs2, start2, notText);
    return start2;
    function start2(code4) {
      return atBreak(code4) ? text7(code4) : notText(code4);
    }
    function notText(code4) {
      if (code4 === null) {
        effects.consume(code4);
        return;
      }
      effects.enter("data");
      effects.consume(code4);
      return data;
    }
    function data(code4) {
      if (atBreak(code4)) {
        effects.exit("data");
        return text7(code4);
      }
      effects.consume(code4);
      return data;
    }
    function atBreak(code4) {
      if (code4 === null) {
        return true;
      }
      const list4 = constructs2[code4];
      let index3 = -1;
      if (list4) {
        while (++index3 < list4.length) {
          const item = list4[index3];
          if (!item.previous || item.previous.call(self2, self2.previous)) {
            return true;
          }
        }
      }
      return false;
    }
  }
}
function createResolver(extraResolver) {
  return resolveAllText;
  function resolveAllText(events, context) {
    let index3 = -1;
    let enter;
    while (++index3 <= events.length) {
      if (enter === void 0) {
        if (events[index3] && events[index3][1].type === "data") {
          enter = index3;
          index3++;
        }
      } else if (!events[index3] || events[index3][1].type !== "data") {
        if (index3 !== enter + 2) {
          events[enter][1].end = events[index3 - 1][1].end;
          events.splice(enter + 2, index3 - enter - 2);
          index3 = enter + 2;
        }
        enter = void 0;
      }
    }
    return extraResolver ? extraResolver(events, context) : events;
  }
}
function resolveAllLineSuffixes(events, context) {
  const editMap = new EditMap();
  let eventIndex = 0;
  while (++eventIndex <= events.length) {
    if ((eventIndex === events.length || events[eventIndex][1].type === "lineEnding") && events[eventIndex - 1][1].type === "data") {
      const data = events[eventIndex - 1][1];
      const chunks = context.sliceStream(data);
      let index3 = chunks.length;
      let bufferIndex = -1;
      let size = 0;
      let tabs;
      while (index3--) {
        const chunk = chunks[index3];
        if (typeof chunk === "string") {
          bufferIndex = chunk.length;
          while (chunk.charCodeAt(bufferIndex - 1) === 32) {
            size++;
            bufferIndex--;
          }
          if (bufferIndex) {
            break;
          }
          bufferIndex = -1;
        } else if (chunk === -2) {
          tabs = true;
          size++;
        } else if (chunk === -1) {
        } else {
          index3++;
          break;
        }
      }
      if (context._contentTypeTextTrailing && eventIndex === events.length) {
        size = 0;
      }
      if (size) {
        const token = {
          type: eventIndex === events.length || tabs || size < 2 ? "lineSuffix" : "hardBreakTrailing",
          start: {
            _bufferIndex: index3 ? bufferIndex : data.start._bufferIndex + bufferIndex,
            _index: data.start._index + index3,
            line: data.end.line,
            column: data.end.column - size,
            offset: data.end.offset - size
          },
          end: {
            ...data.end
          }
        };
        data.end = {
          ...token.start
        };
        if (data.start.offset === data.end.offset) {
          Object.assign(data, token);
        } else {
          editMap.add(eventIndex, 0, [["enter", token, context], ["exit", token, context]]);
        }
      }
      eventIndex++;
    }
  }
  editMap.consume(events);
  return events;
}

// node_modules/micromark/lib/constructs.js
var constructs_exports = {};
__export(constructs_exports, {
  attentionMarkers: () => attentionMarkers,
  contentInitial: () => contentInitial,
  disable: () => disable,
  document: () => document3,
  flow: () => flow2,
  flowInitial: () => flowInitial,
  insideSpan: () => insideSpan,
  string: () => string2,
  text: () => text3
});
var document3 = {
  [42]: list,
  [43]: list,
  [45]: list,
  [48]: list,
  [49]: list,
  [50]: list,
  [51]: list,
  [52]: list,
  [53]: list,
  [54]: list,
  [55]: list,
  [56]: list,
  [57]: list,
  [62]: blockQuote
};
var contentInitial = {
  [91]: definition
};
var flowInitial = {
  [-2]: codeIndented,
  [-1]: codeIndented,
  [32]: codeIndented
};
var flow2 = {
  [35]: headingAtx,
  [42]: thematicBreak,
  [45]: [setextUnderline, thematicBreak],
  [60]: htmlFlow,
  [61]: setextUnderline,
  [95]: thematicBreak,
  [96]: codeFenced,
  [126]: codeFenced
};
var string2 = {
  [38]: characterReference,
  [92]: characterEscape
};
var text3 = {
  [-5]: lineEnding,
  [-4]: lineEnding,
  [-3]: lineEnding,
  [33]: labelStartImage,
  [38]: characterReference,
  [42]: attention,
  [60]: [autolink, htmlText],
  [91]: labelStartLink,
  [92]: [hardBreakEscape, characterEscape],
  [93]: labelEnd,
  [95]: attention,
  [96]: codeText
};
var insideSpan = {
  null: [attention, resolver]
};
var attentionMarkers = {
  null: [42, 95]
};
var disable = {
  null: []
};

// node_modules/micromark/lib/create-tokenizer.js
function createTokenizer(parser, initialize, from) {
  let point4 = {
    _bufferIndex: -1,
    _index: 0,
    line: from && from.line || 1,
    column: from && from.column || 1,
    offset: from && from.offset || 0
  };
  const columnStart = {};
  const resolveAllConstructs = [];
  let chunks = [];
  let stack = [];
  let consumed = true;
  const effects = {
    attempt: constructFactory(onsuccessfulconstruct),
    check: constructFactory(onsuccessfulcheck),
    consume,
    enter,
    exit: exit3,
    interrupt: constructFactory(onsuccessfulcheck, {
      interrupt: true
    })
  };
  const context = {
    code: null,
    containerState: {},
    defineSkip,
    events: [],
    now,
    parser,
    previous: null,
    sliceSerialize,
    sliceStream,
    write
  };
  let state2 = initialize.tokenize.call(context, effects);
  let expectedCode;
  if (initialize.resolveAll) {
    resolveAllConstructs.push(initialize);
  }
  return context;
  function write(slice) {
    chunks = push(chunks, slice);
    main();
    if (chunks[chunks.length - 1] !== null) {
      return [];
    }
    addResult(initialize, 0);
    context.events = resolveAll(resolveAllConstructs, context.events, context);
    return context.events;
  }
  function sliceSerialize(token, expandTabs) {
    return serializeChunks(sliceStream(token), expandTabs);
  }
  function sliceStream(token) {
    return sliceChunks(chunks, token);
  }
  function now() {
    const {
      _bufferIndex,
      _index,
      line,
      column,
      offset
    } = point4;
    return {
      _bufferIndex,
      _index,
      line,
      column,
      offset
    };
  }
  function defineSkip(value) {
    columnStart[value.line] = value.column;
    accountForPotentialSkip();
  }
  function main() {
    while (point4._index < chunks.length) {
      const chunk = chunks[point4._index];
      if (typeof chunk === "string") {
        const chunkIndex = point4._index;
        if (point4._bufferIndex < 0) {
          point4._bufferIndex = 0;
        }
        while (point4._index === chunkIndex && point4._bufferIndex < chunk.length) {
          go(chunk.charCodeAt(point4._bufferIndex));
        }
      } else {
        go(chunk);
      }
    }
  }
  function go(code4) {
    consumed = void 0;
    expectedCode = code4;
    state2 = state2(code4);
  }
  function consume(code4) {
    if (markdownLineEnding(code4)) {
      point4.line++;
      point4.column = 1;
      point4.offset += code4 === -3 ? 2 : 1;
      accountForPotentialSkip();
    } else if (code4 !== -1) {
      point4.column++;
      point4.offset++;
    }
    if (point4._bufferIndex < 0) {
      point4._index++;
    } else {
      point4._bufferIndex++;
      if (point4._bufferIndex === // Points w/ non-negative `_bufferIndex` reference
      // strings.
      /** @type {string} */
      chunks[point4._index].length) {
        point4._bufferIndex = -1;
        point4._index++;
      }
    }
    context.previous = code4;
    consumed = true;
  }
  function enter(type, fields) {
    const token = fields || {};
    token.type = type;
    token.start = now();
    context.events.push(["enter", token, context]);
    stack.push(token);
    return token;
  }
  function exit3(type) {
    const token = stack.pop();
    token.end = now();
    context.events.push(["exit", token, context]);
    return token;
  }
  function onsuccessfulconstruct(construct, info) {
    addResult(construct, info.from);
  }
  function onsuccessfulcheck(_, info) {
    info.restore();
  }
  function constructFactory(onreturn, fields) {
    return hook;
    function hook(constructs2, returnState, bogusState) {
      let listOfConstructs;
      let constructIndex;
      let currentConstruct;
      let info;
      return Array.isArray(constructs2) ? (
        /* c8 ignore next 1 */
        handleListOfConstructs(constructs2)
      ) : "tokenize" in constructs2 ? (
        // Looks like a construct.
        handleListOfConstructs([
          /** @type {Construct} */
          constructs2
        ])
      ) : handleMapOfConstructs(constructs2);
      function handleMapOfConstructs(map3) {
        return start2;
        function start2(code4) {
          const left = code4 !== null && map3[code4];
          const all2 = code4 !== null && map3.null;
          const list4 = [
            // To do: add more extension tests.
            /* c8 ignore next 2 */
            ...Array.isArray(left) ? left : left ? [left] : [],
            ...Array.isArray(all2) ? all2 : all2 ? [all2] : []
          ];
          return handleListOfConstructs(list4)(code4);
        }
      }
      function handleListOfConstructs(list4) {
        listOfConstructs = list4;
        constructIndex = 0;
        if (list4.length === 0) {
          return bogusState;
        }
        return handleConstruct(list4[constructIndex]);
      }
      function handleConstruct(construct) {
        return start2;
        function start2(code4) {
          info = store();
          currentConstruct = construct;
          if (!construct.partial) {
            context.currentConstruct = construct;
          }
          if (construct.name && context.parser.constructs.disable.null.includes(construct.name)) {
            return nok(code4);
          }
          return construct.tokenize.call(
            // If we do have fields, create an object w/ `context` as its
            // prototype.
            // This allows a “live binding”, which is needed for `interrupt`.
            fields ? Object.assign(Object.create(context), fields) : context,
            effects,
            ok3,
            nok
          )(code4);
        }
      }
      function ok3(code4) {
        consumed = true;
        onreturn(currentConstruct, info);
        return returnState;
      }
      function nok(code4) {
        consumed = true;
        info.restore();
        if (++constructIndex < listOfConstructs.length) {
          return handleConstruct(listOfConstructs[constructIndex]);
        }
        return bogusState;
      }
    }
  }
  function addResult(construct, from2) {
    if (construct.resolveAll && !resolveAllConstructs.includes(construct)) {
      resolveAllConstructs.push(construct);
    }
    if (construct.resolve) {
      splice(context.events, from2, context.events.length - from2, construct.resolve(context.events.slice(from2), context));
    }
    if (construct.resolveTo) {
      context.events = construct.resolveTo(context.events, context);
    }
  }
  function store() {
    const startPoint = now();
    const startPrevious = context.previous;
    const startCurrentConstruct = context.currentConstruct;
    const startEventsIndex = context.events.length;
    const startStack = Array.from(stack);
    return {
      from: startEventsIndex,
      restore
    };
    function restore() {
      point4 = startPoint;
      context.previous = startPrevious;
      context.currentConstruct = startCurrentConstruct;
      context.events.length = startEventsIndex;
      stack = startStack;
      accountForPotentialSkip();
    }
  }
  function accountForPotentialSkip() {
    if (point4.line in columnStart && point4.column < 2) {
      point4.column = columnStart[point4.line];
      point4.offset += columnStart[point4.line] - 1;
    }
  }
}
function sliceChunks(chunks, token) {
  const startIndex = token.start._index;
  const startBufferIndex = token.start._bufferIndex;
  const endIndex = token.end._index;
  const endBufferIndex = token.end._bufferIndex;
  let view;
  if (startIndex === endIndex) {
    view = [chunks[startIndex].slice(startBufferIndex, endBufferIndex)];
  } else {
    view = chunks.slice(startIndex, endIndex);
    if (startBufferIndex > -1) {
      const head = view[0];
      if (typeof head === "string") {
        view[0] = head.slice(startBufferIndex);
      } else {
        view.shift();
      }
    }
    if (endBufferIndex > 0) {
      view.push(chunks[endIndex].slice(0, endBufferIndex));
    }
  }
  return view;
}
function serializeChunks(chunks, expandTabs) {
  let index3 = -1;
  const result = [];
  let atTab;
  while (++index3 < chunks.length) {
    const chunk = chunks[index3];
    let value;
    if (typeof chunk === "string") {
      value = chunk;
    } else {
      switch (chunk) {
        case -5: {
          value = "\r";
          break;
        }
        case -4: {
          value = "\n";
          break;
        }
        case -3: {
          value = "\r\n";
          break;
        }
        case -2: {
          value = expandTabs ? " " : "	";
          break;
        }
        case -1: {
          if (!expandTabs && atTab) {
            continue;
          }
          value = " ";
          break;
        }
        default: {
          value = String.fromCharCode(chunk);
        }
      }
    }
    atTab = chunk === -2;
    result.push(value);
  }
  return result.join("");
}

// node_modules/micromark/lib/parse.js
function parse(options) {
  const settings = options || {};
  const constructs2 = (
    /** @type {FullNormalizedExtension} */
    combineExtensions([constructs_exports, ...settings.extensions || []])
  );
  const parser = {
    constructs: constructs2,
    content: create2(content),
    defined: [],
    document: create2(document2),
    flow: create2(flow),
    lazy: {},
    string: create2(string),
    text: create2(text2)
  };
  return parser;
  function create2(initial) {
    return creator;
    function creator(from) {
      return createTokenizer(parser, initial, from);
    }
  }
}

// node_modules/micromark/lib/postprocess.js
function postprocess(events) {
  while (!subtokenize(events)) {
  }
  return events;
}

// node_modules/micromark/lib/preprocess.js
var search = /[\0\t\n\r]/g;
function preprocess() {
  let column = 1;
  let buffer = "";
  let start2 = true;
  let atCarriageReturn;
  return preprocessor;
  function preprocessor(value, encoding, end) {
    value = buffer + (typeof value === "string" ? value.toString() : new TextDecoder(encoding || void 0).decode(value));
    const chunks = [];
    let startPosition = 0;
    buffer = "";
    if (start2) {
      if (value.charCodeAt(0) === 65279) {
        startPosition++;
      }
      start2 = void 0;
    }
    while (startPosition < value.length) {
      search.lastIndex = startPosition;
      const match = search.exec(value);
      const endPosition = match && match.index !== void 0 ? match.index : value.length;
      const code4 = value.charCodeAt(endPosition);
      if (!match) {
        buffer = value.slice(startPosition);
        break;
      }
      if (code4 === 10 && startPosition === endPosition && atCarriageReturn) {
        chunks.push(-3);
        atCarriageReturn = void 0;
      } else {
        if (atCarriageReturn) {
          chunks.push(-5);
          atCarriageReturn = void 0;
        }
        if (startPosition < endPosition) {
          chunks.push(value.slice(startPosition, endPosition));
          column += endPosition - startPosition;
        }
        switch (code4) {
          case 0: {
            chunks.push(65533);
            column++;
            break;
          }
          case 9: {
            const next = Math.ceil(column / 4) * 4;
            chunks.push(-2);
            while (column++ < next) {
              chunks.push(-1);
            }
            break;
          }
          case 10: {
            chunks.push(-4);
            column = 1;
            break;
          }
          default: {
            atCarriageReturn = true;
            column = 1;
          }
        }
      }
      startPosition = endPosition + 1;
    }
    if (end) {
      if (atCarriageReturn) {
        chunks.push(-5);
      }
      if (buffer) {
        chunks.push(buffer);
      }
      chunks.push(null);
    }
    return chunks;
  }
}

// node_modules/micromark-util-decode-string/index.js
var characterEscapeOrReference = /\\([!-/:-@[-`{-~])|&(#(?:\d{1,7}|x[\da-f]{1,6})|[\da-z]{1,31});/gi;
function decodeString(value) {
  return value.replace(characterEscapeOrReference, decode);
}
function decode($0, $1, $2) {
  if ($1) {
    return $1;
  }
  const head = $2.charCodeAt(0);
  if (head === 35) {
    const head2 = $2.charCodeAt(1);
    const hex = head2 === 120 || head2 === 88;
    return decodeNumericCharacterReference($2.slice(hex ? 2 : 1), hex ? 16 : 10);
  }
  return decodeNamedCharacterReference($2) || $0;
}

// node_modules/mdast-util-from-markdown/lib/index.js
var own2 = {}.hasOwnProperty;
function fromMarkdown(value, encoding, options) {
  if (encoding && typeof encoding === "object") {
    options = encoding;
    encoding = void 0;
  }
  return compiler(options)(postprocess(parse(options).document().write(preprocess()(value, encoding, true))));
}
function compiler(options) {
  const config = {
    afterExit: [],
    beforeEnter: [],
    transforms: [],
    canContainEols: ["emphasis", "fragment", "heading", "paragraph", "strong"],
    enter: {
      autolink: opener(link3),
      autolinkProtocol: onenterdata,
      autolinkEmail: onenterdata,
      atxHeading: opener(heading3),
      blockQuote: opener(blockQuote2),
      characterEscape: onenterdata,
      characterReference: onenterdata,
      codeFenced: opener(codeFlow),
      codeFencedFenceInfo: buffer,
      codeFencedFenceMeta: buffer,
      codeIndented: opener(codeFlow, buffer),
      codeText: opener(codeText2, buffer),
      codeTextData: onenterdata,
      data: onenterdata,
      codeFlowValue: onenterdata,
      definition: opener(definition3),
      definitionDestinationString: buffer,
      definitionLabelString: buffer,
      definitionTitleString: buffer,
      emphasis: opener(emphasis3),
      hardBreakEscape: opener(hardBreak3),
      hardBreakTrailing: opener(hardBreak3),
      htmlFlow: opener(html5, buffer),
      htmlFlowData: onenterdata,
      htmlText: opener(html5, buffer),
      htmlTextData: onenterdata,
      image: opener(image3),
      label: buffer,
      link: opener(link3),
      listItem: opener(listItem3),
      listItemValue: onenterlistitemvalue,
      listOrdered: opener(list4, onenterlistordered),
      listUnordered: opener(list4),
      paragraph: opener(paragraph3),
      reference: onenterreference,
      referenceString: buffer,
      resourceDestinationString: buffer,
      resourceTitleString: buffer,
      setextHeading: opener(heading3),
      strong: opener(strong3),
      thematicBreak: opener(thematicBreak4)
    },
    exit: {
      atxHeading: closer(),
      atxHeadingSequence: onexitatxheadingsequence,
      autolink: closer(),
      autolinkEmail: onexitautolinkemail,
      autolinkProtocol: onexitautolinkprotocol,
      blockQuote: closer(),
      characterEscapeValue: onexitdata,
      characterReferenceMarkerHexadecimal: onexitcharacterreferencemarker,
      characterReferenceMarkerNumeric: onexitcharacterreferencemarker,
      characterReferenceValue: onexitcharacterreferencevalue,
      characterReference: onexitcharacterreference,
      codeFenced: closer(onexitcodefenced),
      codeFencedFence: onexitcodefencedfence,
      codeFencedFenceInfo: onexitcodefencedfenceinfo,
      codeFencedFenceMeta: onexitcodefencedfencemeta,
      codeFlowValue: onexitdata,
      codeIndented: closer(onexitcodeindented),
      codeText: closer(onexitcodetext),
      codeTextData: onexitdata,
      data: onexitdata,
      definition: closer(),
      definitionDestinationString: onexitdefinitiondestinationstring,
      definitionLabelString: onexitdefinitionlabelstring,
      definitionTitleString: onexitdefinitiontitlestring,
      emphasis: closer(),
      hardBreakEscape: closer(onexithardbreak),
      hardBreakTrailing: closer(onexithardbreak),
      htmlFlow: closer(onexithtmlflow),
      htmlFlowData: onexitdata,
      htmlText: closer(onexithtmltext),
      htmlTextData: onexitdata,
      image: closer(onexitimage),
      label: onexitlabel,
      labelText: onexitlabeltext,
      lineEnding: onexitlineending,
      link: closer(onexitlink),
      listItem: closer(),
      listOrdered: closer(),
      listUnordered: closer(),
      paragraph: closer(),
      referenceString: onexitreferencestring,
      resourceDestinationString: onexitresourcedestinationstring,
      resourceTitleString: onexitresourcetitlestring,
      resource: onexitresource,
      setextHeading: closer(onexitsetextheading),
      setextHeadingLineSequence: onexitsetextheadinglinesequence,
      setextHeadingText: onexitsetextheadingtext,
      strong: closer(),
      thematicBreak: closer()
    }
  };
  configure(config, (options || {}).mdastExtensions || []);
  const data = {};
  return compile;
  function compile(events) {
    let tree = {
      type: "root",
      children: [],
      position: void 0
    };
    const context = {
      stack: [tree],
      tokenStack: [],
      config,
      enter,
      exit: exit3,
      buffer,
      resume,
      data
    };
    const listStack = [];
    const prepared = [];
    let index3 = -1;
    while (++index3 < events.length) {
      prepared.push(events[index3]);
      if (events[index3][1].type === "listOrdered" || events[index3][1].type === "listUnordered") {
        if (events[index3][0] === "enter") {
          listStack.push(prepared.length - 1);
        } else {
          const tail = listStack.pop();
          prepareList(prepared, tail);
        }
      }
    }
    events = prepared;
    index3 = -1;
    while (++index3 < events.length) {
      const handler = config[events[index3][0]];
      if (events[index3][0] === "enter" && config.beforeEnter.length > 0) {
        callListeners(config.beforeEnter, {
          ...context,
          sliceSerialize: events[index3][2].sliceSerialize
        }, events[index3][1]);
      }
      if (own2.call(handler, events[index3][1].type)) {
        handler[events[index3][1].type].call({
          ...context,
          sliceSerialize: events[index3][2].sliceSerialize
        }, events[index3][1]);
      }
      if (events[index3][0] === "exit" && config.afterExit.length > 0) {
        callListeners(config.afterExit, {
          ...context,
          sliceSerialize: events[index3][2].sliceSerialize
        }, events[index3][1]);
      }
    }
    if (context.tokenStack.length > 0) {
      const tail = context.tokenStack[context.tokenStack.length - 1];
      const handler = tail[1] || defaultOnError;
      handler.call(context, void 0, tail[0]);
    }
    tree.position = {
      start: point3(events.length > 0 ? events[0][1].start : {
        line: 1,
        column: 1,
        offset: 0
      }),
      end: point3(events.length > 0 ? events[events.length - 2][1].end : {
        line: 1,
        column: 1,
        offset: 0
      })
    };
    index3 = -1;
    while (++index3 < config.transforms.length) {
      tree = config.transforms[index3](tree) || tree;
    }
    return tree;
  }
  function callListeners(listeners2, context, token) {
    let index3 = -1;
    while (++index3 < listeners2.length) {
      listeners2[index3].call(context, token);
    }
  }
  function prepareList(events, start2) {
    const end = events.length - 1;
    let index3 = start2 - 1;
    let containerBalance = -1;
    let listSpread = false;
    let listItem4;
    let lineIndex;
    let firstBlankLineIndex;
    let atMarker;
    const insertions = [];
    while (++index3 <= end) {
      const event = events[index3];
      switch (event[1].type) {
        case "listUnordered":
        case "listOrdered":
        case "blockQuote": {
          if (event[0] === "enter") {
            containerBalance++;
          } else {
            containerBalance--;
          }
          atMarker = void 0;
          break;
        }
        case "lineEndingBlank": {
          if (event[0] === "enter") {
            if (listItem4 && !atMarker && !containerBalance && !firstBlankLineIndex) {
              firstBlankLineIndex = index3;
            }
            atMarker = void 0;
          }
          break;
        }
        case "linePrefix":
        case "listItemValue":
        case "listItemMarker":
        case "listItemPrefix":
        case "listItemPrefixWhitespace": {
          break;
        }
        default: {
          atMarker = void 0;
        }
      }
      if (!containerBalance && event[0] === "enter" && event[1].type === "listItemPrefix" || containerBalance === -1 && event[0] === "exit" && (event[1].type === "listUnordered" || event[1].type === "listOrdered")) {
        if (listItem4) {
          let tailIndex = index3;
          lineIndex = void 0;
          while (tailIndex--) {
            const tailEvent = events[tailIndex];
            if (tailEvent[1].type === "lineEnding" || tailEvent[1].type === "lineEndingBlank") {
              if (tailEvent[0] === "exit") continue;
              if (lineIndex) {
                events[lineIndex][1].type = "lineEndingBlank";
                listSpread = true;
              }
              tailEvent[1].type = "lineEnding";
              lineIndex = tailIndex;
            } else if (tailEvent[1].type === "linePrefix" || tailEvent[1].type === "blockQuotePrefix" || tailEvent[1].type === "blockQuotePrefixWhitespace" || tailEvent[1].type === "blockQuoteMarker" || tailEvent[1].type === "listItemIndent") {
            } else {
              break;
            }
          }
          if (firstBlankLineIndex && (!lineIndex || firstBlankLineIndex < lineIndex)) {
            listItem4._spread = true;
          }
          listItem4.end = Object.assign({}, lineIndex ? events[lineIndex][1].start : event[1].end);
          insertions.push({
            at: lineIndex || index3,
            event: ["exit", listItem4, event[2]]
          });
        }
        if (event[1].type === "listItemPrefix") {
          const item = {
            type: "listItem",
            _spread: false,
            start: Object.assign({}, event[1].start),
            // @ts-expect-error: we’ll add `end` in a second.
            end: void 0
          };
          listItem4 = item;
          insertions.push({
            at: index3,
            event: ["enter", item, event[2]]
          });
          firstBlankLineIndex = void 0;
          atMarker = true;
        }
      }
    }
    const listEvents = events.splice(start2);
    let insertion = 0;
    index3 = -1;
    while (++index3 < listEvents.length) {
      while (insertion < insertions.length && insertions[insertion].at === start2 + index3) {
        events.push(insertions[insertion++].event);
      }
      events.push(listEvents[index3]);
    }
    events[start2][1]._spread = listSpread;
  }
  function opener(create2, and) {
    return open;
    function open(token) {
      enter.call(this, create2(token), token);
      if (and) and.call(this, token);
    }
  }
  function buffer() {
    this.stack.push({
      type: "fragment",
      children: []
    });
  }
  function enter(node2, token, errorHandler) {
    const parent = this.stack[this.stack.length - 1];
    const siblings = parent.children;
    siblings.push(node2);
    this.stack.push(node2);
    this.tokenStack.push([token, errorHandler || void 0]);
    node2.position = {
      start: point3(token.start),
      // @ts-expect-error: `end` will be patched later.
      end: void 0
    };
  }
  function closer(and) {
    return close;
    function close(token) {
      if (and) and.call(this, token);
      exit3.call(this, token);
    }
  }
  function exit3(token, onExitError) {
    const node2 = this.stack.pop();
    const open = this.tokenStack.pop();
    if (!open) {
      throw new Error("Cannot close `" + token.type + "` (" + stringifyPosition({
        start: token.start,
        end: token.end
      }) + "): it\u2019s not open");
    }
    if (open[0].type !== token.type) {
      if (onExitError) {
        onExitError.call(this, token, open[0]);
      } else {
        const handler = open[1] || defaultOnError;
        handler.call(this, token, open[0]);
      }
    }
    node2.position.end = point3(token.end);
  }
  function resume() {
    return toString(this.stack.pop());
  }
  function onenterlistordered() {
    this.data.expectingFirstListItemValue = true;
  }
  function onenterlistitemvalue(token) {
    if (this.data.expectingFirstListItemValue) {
      const ancestor = this.stack[this.stack.length - 2];
      ancestor.start = Number.parseInt(this.sliceSerialize(token), 10);
      this.data.expectingFirstListItemValue = void 0;
    }
  }
  function onexitcodefencedfenceinfo() {
    const data2 = this.resume();
    const node2 = this.stack[this.stack.length - 1];
    node2.lang = data2;
  }
  function onexitcodefencedfencemeta() {
    const data2 = this.resume();
    const node2 = this.stack[this.stack.length - 1];
    node2.meta = data2;
  }
  function onexitcodefencedfence() {
    if (this.data.flowCodeInside) return;
    this.buffer();
    this.data.flowCodeInside = true;
  }
  function onexitcodefenced() {
    const data2 = this.resume();
    const node2 = this.stack[this.stack.length - 1];
    node2.value = data2.replace(/^(\r?\n|\r)|(\r?\n|\r)$/g, "");
    this.data.flowCodeInside = void 0;
  }
  function onexitcodeindented() {
    const data2 = this.resume();
    const node2 = this.stack[this.stack.length - 1];
    node2.value = data2.replace(/(\r?\n|\r)$/g, "");
  }
  function onexitdefinitionlabelstring(token) {
    const label = this.resume();
    const node2 = this.stack[this.stack.length - 1];
    node2.label = label;
    node2.identifier = normalizeIdentifier(this.sliceSerialize(token)).toLowerCase();
  }
  function onexitdefinitiontitlestring() {
    const data2 = this.resume();
    const node2 = this.stack[this.stack.length - 1];
    node2.title = data2;
  }
  function onexitdefinitiondestinationstring() {
    const data2 = this.resume();
    const node2 = this.stack[this.stack.length - 1];
    node2.url = data2;
  }
  function onexitatxheadingsequence(token) {
    const node2 = this.stack[this.stack.length - 1];
    if (!node2.depth) {
      const depth = this.sliceSerialize(token).length;
      node2.depth = depth;
    }
  }
  function onexitsetextheadingtext() {
    this.data.setextHeadingSlurpLineEnding = true;
  }
  function onexitsetextheadinglinesequence(token) {
    const node2 = this.stack[this.stack.length - 1];
    node2.depth = this.sliceSerialize(token).codePointAt(0) === 61 ? 1 : 2;
  }
  function onexitsetextheading() {
    this.data.setextHeadingSlurpLineEnding = void 0;
  }
  function onenterdata(token) {
    const node2 = this.stack[this.stack.length - 1];
    const siblings = node2.children;
    let tail = siblings[siblings.length - 1];
    if (!tail || tail.type !== "text") {
      tail = text7();
      tail.position = {
        start: point3(token.start),
        // @ts-expect-error: we’ll add `end` later.
        end: void 0
      };
      siblings.push(tail);
    }
    this.stack.push(tail);
  }
  function onexitdata(token) {
    const tail = this.stack.pop();
    tail.value += this.sliceSerialize(token);
    tail.position.end = point3(token.end);
  }
  function onexitlineending(token) {
    const context = this.stack[this.stack.length - 1];
    if (this.data.atHardBreak) {
      const tail = context.children[context.children.length - 1];
      tail.position.end = point3(token.end);
      this.data.atHardBreak = void 0;
      return;
    }
    if (!this.data.setextHeadingSlurpLineEnding && config.canContainEols.includes(context.type)) {
      onenterdata.call(this, token);
      onexitdata.call(this, token);
    }
  }
  function onexithardbreak() {
    this.data.atHardBreak = true;
  }
  function onexithtmlflow() {
    const data2 = this.resume();
    const node2 = this.stack[this.stack.length - 1];
    node2.value = data2;
  }
  function onexithtmltext() {
    const data2 = this.resume();
    const node2 = this.stack[this.stack.length - 1];
    node2.value = data2;
  }
  function onexitcodetext() {
    const data2 = this.resume();
    const node2 = this.stack[this.stack.length - 1];
    node2.value = data2;
  }
  function onexitlink() {
    const node2 = this.stack[this.stack.length - 1];
    if (this.data.inReference) {
      const referenceType = this.data.referenceType || "shortcut";
      node2.type += "Reference";
      node2.referenceType = referenceType;
      delete node2.url;
      delete node2.title;
    } else {
      delete node2.identifier;
      delete node2.label;
    }
    this.data.referenceType = void 0;
  }
  function onexitimage() {
    const node2 = this.stack[this.stack.length - 1];
    if (this.data.inReference) {
      const referenceType = this.data.referenceType || "shortcut";
      node2.type += "Reference";
      node2.referenceType = referenceType;
      delete node2.url;
      delete node2.title;
    } else {
      delete node2.identifier;
      delete node2.label;
    }
    this.data.referenceType = void 0;
  }
  function onexitlabeltext(token) {
    const string3 = this.sliceSerialize(token);
    const ancestor = this.stack[this.stack.length - 2];
    ancestor.label = decodeString(string3);
    ancestor.identifier = normalizeIdentifier(string3).toLowerCase();
  }
  function onexitlabel() {
    const fragment = this.stack[this.stack.length - 1];
    const value = this.resume();
    const node2 = this.stack[this.stack.length - 1];
    this.data.inReference = true;
    if (node2.type === "link") {
      const children = fragment.children;
      node2.children = children;
    } else {
      node2.alt = value;
    }
  }
  function onexitresourcedestinationstring() {
    const data2 = this.resume();
    const node2 = this.stack[this.stack.length - 1];
    node2.url = data2;
  }
  function onexitresourcetitlestring() {
    const data2 = this.resume();
    const node2 = this.stack[this.stack.length - 1];
    node2.title = data2;
  }
  function onexitresource() {
    this.data.inReference = void 0;
  }
  function onenterreference() {
    this.data.referenceType = "collapsed";
  }
  function onexitreferencestring(token) {
    const label = this.resume();
    const node2 = this.stack[this.stack.length - 1];
    node2.label = label;
    node2.identifier = normalizeIdentifier(this.sliceSerialize(token)).toLowerCase();
    this.data.referenceType = "full";
  }
  function onexitcharacterreferencemarker(token) {
    this.data.characterReferenceType = token.type;
  }
  function onexitcharacterreferencevalue(token) {
    const data2 = this.sliceSerialize(token);
    const type = this.data.characterReferenceType;
    let value;
    if (type) {
      value = decodeNumericCharacterReference(data2, type === "characterReferenceMarkerNumeric" ? 10 : 16);
      this.data.characterReferenceType = void 0;
    } else {
      const result = decodeNamedCharacterReference(data2);
      value = result;
    }
    const tail = this.stack[this.stack.length - 1];
    tail.value += value;
  }
  function onexitcharacterreference(token) {
    const tail = this.stack.pop();
    tail.position.end = point3(token.end);
  }
  function onexitautolinkprotocol(token) {
    onexitdata.call(this, token);
    const node2 = this.stack[this.stack.length - 1];
    node2.url = this.sliceSerialize(token);
  }
  function onexitautolinkemail(token) {
    onexitdata.call(this, token);
    const node2 = this.stack[this.stack.length - 1];
    node2.url = "mailto:" + this.sliceSerialize(token);
  }
  function blockQuote2() {
    return {
      type: "blockquote",
      children: [],
      position: void 0
    };
  }
  function codeFlow() {
    return {
      type: "code",
      lang: null,
      meta: null,
      value: "",
      position: void 0
    };
  }
  function codeText2() {
    return {
      type: "inlineCode",
      value: "",
      position: void 0
    };
  }
  function definition3() {
    return {
      type: "definition",
      identifier: "",
      label: null,
      title: null,
      url: "",
      position: void 0
    };
  }
  function emphasis3() {
    return {
      type: "emphasis",
      children: [],
      position: void 0
    };
  }
  function heading3() {
    return {
      type: "heading",
      // @ts-expect-error `depth` will be set later.
      depth: 0,
      children: [],
      position: void 0
    };
  }
  function hardBreak3() {
    return {
      type: "break",
      position: void 0
    };
  }
  function html5() {
    return {
      type: "html",
      value: "",
      position: void 0
    };
  }
  function image3() {
    return {
      type: "image",
      title: null,
      url: "",
      alt: null,
      position: void 0
    };
  }
  function link3() {
    return {
      type: "link",
      title: null,
      url: "",
      children: [],
      position: void 0
    };
  }
  function list4(token) {
    return {
      type: "list",
      ordered: token.type === "listOrdered",
      start: null,
      spread: token._spread,
      children: [],
      position: void 0
    };
  }
  function listItem3(token) {
    return {
      type: "listItem",
      spread: token._spread,
      checked: null,
      children: [],
      position: void 0
    };
  }
  function paragraph3() {
    return {
      type: "paragraph",
      children: [],
      position: void 0
    };
  }
  function strong3() {
    return {
      type: "strong",
      children: [],
      position: void 0
    };
  }
  function text7() {
    return {
      type: "text",
      value: "",
      position: void 0
    };
  }
  function thematicBreak4() {
    return {
      type: "thematicBreak",
      position: void 0
    };
  }
}
function point3(d) {
  return {
    line: d.line,
    column: d.column,
    offset: d.offset
  };
}
function configure(combined, extensions) {
  let index3 = -1;
  while (++index3 < extensions.length) {
    const value = extensions[index3];
    if (Array.isArray(value)) {
      configure(combined, value);
    } else {
      extension(combined, value);
    }
  }
}
function extension(combined, extension2) {
  let key;
  for (key in extension2) {
    if (own2.call(extension2, key)) {
      switch (key) {
        case "canContainEols": {
          const right = extension2[key];
          if (right) {
            combined[key].push(...right);
          }
          break;
        }
        case "transforms": {
          const right = extension2[key];
          if (right) {
            combined[key].push(...right);
          }
          break;
        }
        case "afterExit":
        case "beforeEnter": {
          const right = extension2[key];
          if (right) {
            combined[key].push(right);
          }
          break;
        }
        case "enter":
        case "exit": {
          const right = extension2[key];
          if (right) {
            Object.assign(combined[key], right);
          }
          break;
        }
      }
    }
  }
}
function defaultOnError(left, right) {
  if (left) {
    throw new Error("Cannot close `" + left.type + "` (" + stringifyPosition({
      start: left.start,
      end: left.end
    }) + "): a different token (`" + right.type + "`, " + stringifyPosition({
      start: right.start,
      end: right.end
    }) + ") is open");
  }
  throw new Error("Cannot close document, a token (`" + right.type + "`, " + stringifyPosition({
    start: right.start,
    end: right.end
  }) + ") is still open");
}

// node_modules/remark-parse/lib/index.js
function remarkParse(options) {
  const self2 = this;
  self2.parser = parser;
  function parser(doc) {
    return fromMarkdown(doc, {
      ...self2.data("settings"),
      ...options,
      // Note: these options are not in the readme.
      // The goal is for them to be set by plugins on `data` instead of being
      // passed by users.
      extensions: self2.data("micromarkExtensions") || [],
      mdastExtensions: self2.data("fromMarkdownExtensions") || []
    });
  }
}

// node_modules/mdast-util-to-hast/lib/handlers/blockquote.js
function blockquote(state2, node2) {
  const result = {
    type: "element",
    tagName: "blockquote",
    properties: {},
    children: state2.wrap(state2.all(node2), true)
  };
  state2.patch(node2, result);
  return state2.applyData(node2, result);
}

// node_modules/mdast-util-to-hast/lib/handlers/break.js
function hardBreak(state2, node2) {
  const result = { type: "element", tagName: "br", properties: {}, children: [] };
  state2.patch(node2, result);
  return [state2.applyData(node2, result), { type: "text", value: "\n" }];
}

// node_modules/mdast-util-to-hast/lib/handlers/code.js
function code(state2, node2) {
  const value = node2.value ? node2.value + "\n" : "";
  const properties = {};
  const language = node2.lang ? node2.lang.split(/\s+/) : [];
  if (language.length > 0) {
    properties.className = ["language-" + language[0]];
  }
  let result = {
    type: "element",
    tagName: "code",
    properties,
    children: [{ type: "text", value }]
  };
  if (node2.meta) {
    result.data = { meta: node2.meta };
  }
  state2.patch(node2, result);
  result = state2.applyData(node2, result);
  result = { type: "element", tagName: "pre", properties: {}, children: [result] };
  state2.patch(node2, result);
  return result;
}

// node_modules/mdast-util-to-hast/lib/handlers/delete.js
function strikethrough(state2, node2) {
  const result = {
    type: "element",
    tagName: "del",
    properties: {},
    children: state2.all(node2)
  };
  state2.patch(node2, result);
  return state2.applyData(node2, result);
}

// node_modules/mdast-util-to-hast/lib/handlers/emphasis.js
function emphasis(state2, node2) {
  const result = {
    type: "element",
    tagName: "em",
    properties: {},
    children: state2.all(node2)
  };
  state2.patch(node2, result);
  return state2.applyData(node2, result);
}

// node_modules/mdast-util-to-hast/lib/handlers/footnote-reference.js
function footnoteReference(state2, node2) {
  const clobberPrefix = typeof state2.options.clobberPrefix === "string" ? state2.options.clobberPrefix : "user-content-";
  const id = String(node2.identifier).toUpperCase();
  const safeId = normalizeUri(id.toLowerCase());
  const index3 = state2.footnoteOrder.indexOf(id);
  let counter;
  let reuseCounter = state2.footnoteCounts.get(id);
  if (reuseCounter === void 0) {
    reuseCounter = 0;
    state2.footnoteOrder.push(id);
    counter = state2.footnoteOrder.length;
  } else {
    counter = index3 + 1;
  }
  reuseCounter += 1;
  state2.footnoteCounts.set(id, reuseCounter);
  const link3 = {
    type: "element",
    tagName: "a",
    properties: {
      href: "#" + clobberPrefix + "fn-" + safeId,
      id: clobberPrefix + "fnref-" + safeId + (reuseCounter > 1 ? "-" + reuseCounter : ""),
      dataFootnoteRef: true,
      ariaDescribedBy: ["footnote-label"]
    },
    children: [{ type: "text", value: String(counter) }]
  };
  state2.patch(node2, link3);
  const sup = {
    type: "element",
    tagName: "sup",
    properties: {},
    children: [link3]
  };
  state2.patch(node2, sup);
  return state2.applyData(node2, sup);
}

// node_modules/mdast-util-to-hast/lib/handlers/heading.js
function heading(state2, node2) {
  const result = {
    type: "element",
    tagName: "h" + node2.depth,
    properties: {},
    children: state2.all(node2)
  };
  state2.patch(node2, result);
  return state2.applyData(node2, result);
}

// node_modules/mdast-util-to-hast/lib/handlers/html.js
function html3(state2, node2) {
  if (state2.options.allowDangerousHtml) {
    const result = { type: "raw", value: node2.value };
    state2.patch(node2, result);
    return state2.applyData(node2, result);
  }
  return void 0;
}

// node_modules/mdast-util-to-hast/lib/revert.js
function revert(state2, node2) {
  const subtype = node2.referenceType;
  let suffix = "]";
  if (subtype === "collapsed") {
    suffix += "[]";
  } else if (subtype === "full") {
    suffix += "[" + (node2.label || node2.identifier) + "]";
  }
  if (node2.type === "imageReference") {
    return [{ type: "text", value: "![" + node2.alt + suffix }];
  }
  const contents = state2.all(node2);
  const head = contents[0];
  if (head && head.type === "text") {
    head.value = "[" + head.value;
  } else {
    contents.unshift({ type: "text", value: "[" });
  }
  const tail = contents[contents.length - 1];
  if (tail && tail.type === "text") {
    tail.value += suffix;
  } else {
    contents.push({ type: "text", value: suffix });
  }
  return contents;
}

// node_modules/mdast-util-to-hast/lib/handlers/image-reference.js
function imageReference(state2, node2) {
  const id = String(node2.identifier).toUpperCase();
  const definition3 = state2.definitionById.get(id);
  if (!definition3) {
    return revert(state2, node2);
  }
  const properties = { src: normalizeUri(definition3.url || ""), alt: node2.alt };
  if (definition3.title !== null && definition3.title !== void 0) {
    properties.title = definition3.title;
  }
  const result = { type: "element", tagName: "img", properties, children: [] };
  state2.patch(node2, result);
  return state2.applyData(node2, result);
}

// node_modules/mdast-util-to-hast/lib/handlers/image.js
function image(state2, node2) {
  const properties = { src: normalizeUri(node2.url) };
  if (node2.alt !== null && node2.alt !== void 0) {
    properties.alt = node2.alt;
  }
  if (node2.title !== null && node2.title !== void 0) {
    properties.title = node2.title;
  }
  const result = { type: "element", tagName: "img", properties, children: [] };
  state2.patch(node2, result);
  return state2.applyData(node2, result);
}

// node_modules/mdast-util-to-hast/lib/handlers/inline-code.js
function inlineCode(state2, node2) {
  const text7 = { type: "text", value: node2.value.replace(/\r?\n|\r/g, " ") };
  state2.patch(node2, text7);
  const result = {
    type: "element",
    tagName: "code",
    properties: {},
    children: [text7]
  };
  state2.patch(node2, result);
  return state2.applyData(node2, result);
}

// node_modules/mdast-util-to-hast/lib/handlers/link-reference.js
function linkReference(state2, node2) {
  const id = String(node2.identifier).toUpperCase();
  const definition3 = state2.definitionById.get(id);
  if (!definition3) {
    return revert(state2, node2);
  }
  const properties = { href: normalizeUri(definition3.url || "") };
  if (definition3.title !== null && definition3.title !== void 0) {
    properties.title = definition3.title;
  }
  const result = {
    type: "element",
    tagName: "a",
    properties,
    children: state2.all(node2)
  };
  state2.patch(node2, result);
  return state2.applyData(node2, result);
}

// node_modules/mdast-util-to-hast/lib/handlers/link.js
function link(state2, node2) {
  const properties = { href: normalizeUri(node2.url) };
  if (node2.title !== null && node2.title !== void 0) {
    properties.title = node2.title;
  }
  const result = {
    type: "element",
    tagName: "a",
    properties,
    children: state2.all(node2)
  };
  state2.patch(node2, result);
  return state2.applyData(node2, result);
}

// node_modules/mdast-util-to-hast/lib/handlers/list-item.js
function listItem(state2, node2, parent) {
  const results = state2.all(node2);
  const loose = parent ? listLoose(parent) : listItemLoose(node2);
  const properties = {};
  const children = [];
  if (typeof node2.checked === "boolean") {
    const head = results[0];
    let paragraph3;
    if (head && head.type === "element" && head.tagName === "p") {
      paragraph3 = head;
    } else {
      paragraph3 = { type: "element", tagName: "p", properties: {}, children: [] };
      results.unshift(paragraph3);
    }
    if (paragraph3.children.length > 0) {
      paragraph3.children.unshift({ type: "text", value: " " });
    }
    paragraph3.children.unshift({
      type: "element",
      tagName: "input",
      properties: { type: "checkbox", checked: node2.checked, disabled: true },
      children: []
    });
    properties.className = ["task-list-item"];
  }
  let index3 = -1;
  while (++index3 < results.length) {
    const child = results[index3];
    if (loose || index3 !== 0 || child.type !== "element" || child.tagName !== "p") {
      children.push({ type: "text", value: "\n" });
    }
    if (child.type === "element" && child.tagName === "p" && !loose) {
      children.push(...child.children);
    } else {
      children.push(child);
    }
  }
  const tail = results[results.length - 1];
  if (tail && (loose || tail.type !== "element" || tail.tagName !== "p")) {
    children.push({ type: "text", value: "\n" });
  }
  const result = { type: "element", tagName: "li", properties, children };
  state2.patch(node2, result);
  return state2.applyData(node2, result);
}
function listLoose(node2) {
  let loose = false;
  if (node2.type === "list") {
    loose = node2.spread || false;
    const children = node2.children;
    let index3 = -1;
    while (!loose && ++index3 < children.length) {
      loose = listItemLoose(children[index3]);
    }
  }
  return loose;
}
function listItemLoose(node2) {
  const spread = node2.spread;
  return spread === null || spread === void 0 ? node2.children.length > 1 : spread;
}

// node_modules/mdast-util-to-hast/lib/handlers/list.js
function list2(state2, node2) {
  const properties = {};
  const results = state2.all(node2);
  let index3 = -1;
  if (typeof node2.start === "number" && node2.start !== 1) {
    properties.start = node2.start;
  }
  while (++index3 < results.length) {
    const child = results[index3];
    if (child.type === "element" && child.tagName === "li" && child.properties && Array.isArray(child.properties.className) && child.properties.className.includes("task-list-item")) {
      properties.className = ["contains-task-list"];
      break;
    }
  }
  const result = {
    type: "element",
    tagName: node2.ordered ? "ol" : "ul",
    properties,
    children: state2.wrap(results, true)
  };
  state2.patch(node2, result);
  return state2.applyData(node2, result);
}

// node_modules/mdast-util-to-hast/lib/handlers/paragraph.js
function paragraph(state2, node2) {
  const result = {
    type: "element",
    tagName: "p",
    properties: {},
    children: state2.all(node2)
  };
  state2.patch(node2, result);
  return state2.applyData(node2, result);
}

// node_modules/mdast-util-to-hast/lib/handlers/root.js
function root2(state2, node2) {
  const result = { type: "root", children: state2.wrap(state2.all(node2)) };
  state2.patch(node2, result);
  return state2.applyData(node2, result);
}

// node_modules/mdast-util-to-hast/lib/handlers/strong.js
function strong(state2, node2) {
  const result = {
    type: "element",
    tagName: "strong",
    properties: {},
    children: state2.all(node2)
  };
  state2.patch(node2, result);
  return state2.applyData(node2, result);
}

// node_modules/mdast-util-to-hast/lib/handlers/table.js
function table(state2, node2) {
  const rows = state2.all(node2);
  const firstRow = rows.shift();
  const tableContent = [];
  if (firstRow) {
    const head = {
      type: "element",
      tagName: "thead",
      properties: {},
      children: state2.wrap([firstRow], true)
    };
    state2.patch(node2.children[0], head);
    tableContent.push(head);
  }
  if (rows.length > 0) {
    const body = {
      type: "element",
      tagName: "tbody",
      properties: {},
      children: state2.wrap(rows, true)
    };
    const start2 = pointStart(node2.children[1]);
    const end = pointEnd(node2.children[node2.children.length - 1]);
    if (start2 && end) body.position = { start: start2, end };
    tableContent.push(body);
  }
  const result = {
    type: "element",
    tagName: "table",
    properties: {},
    children: state2.wrap(tableContent, true)
  };
  state2.patch(node2, result);
  return state2.applyData(node2, result);
}

// node_modules/mdast-util-to-hast/lib/handlers/table-row.js
function tableRow(state2, node2, parent) {
  const siblings = parent ? parent.children : void 0;
  const rowIndex = siblings ? siblings.indexOf(node2) : 1;
  const tagName = rowIndex === 0 ? "th" : "td";
  const align = parent && parent.type === "table" ? parent.align : void 0;
  const length = align ? align.length : node2.children.length;
  let cellIndex = -1;
  const cells = [];
  while (++cellIndex < length) {
    const cell = node2.children[cellIndex];
    const properties = {};
    const alignValue = align ? align[cellIndex] : void 0;
    if (alignValue) {
      properties.align = alignValue;
    }
    let result2 = { type: "element", tagName, properties, children: [] };
    if (cell) {
      result2.children = state2.all(cell);
      state2.patch(cell, result2);
      result2 = state2.applyData(cell, result2);
    }
    cells.push(result2);
  }
  const result = {
    type: "element",
    tagName: "tr",
    properties: {},
    children: state2.wrap(cells, true)
  };
  state2.patch(node2, result);
  return state2.applyData(node2, result);
}

// node_modules/mdast-util-to-hast/lib/handlers/table-cell.js
function tableCell(state2, node2) {
  const result = {
    type: "element",
    tagName: "td",
    // Assume body cell.
    properties: {},
    children: state2.all(node2)
  };
  state2.patch(node2, result);
  return state2.applyData(node2, result);
}

// node_modules/trim-lines/index.js
var tab = 9;
var space = 32;
function trimLines(value) {
  const source = String(value);
  const search2 = /\r?\n|\r/g;
  let match = search2.exec(source);
  let last = 0;
  const lines = [];
  while (match) {
    lines.push(
      trimLine(source.slice(last, match.index), last > 0, true),
      match[0]
    );
    last = match.index + match[0].length;
    match = search2.exec(source);
  }
  lines.push(trimLine(source.slice(last), last > 0, false));
  return lines.join("");
}
function trimLine(value, start2, end) {
  let startIndex = 0;
  let endIndex = value.length;
  if (start2) {
    let code4 = value.codePointAt(startIndex);
    while (code4 === tab || code4 === space) {
      startIndex++;
      code4 = value.codePointAt(startIndex);
    }
  }
  if (end) {
    let code4 = value.codePointAt(endIndex - 1);
    while (code4 === tab || code4 === space) {
      endIndex--;
      code4 = value.codePointAt(endIndex - 1);
    }
  }
  return endIndex > startIndex ? value.slice(startIndex, endIndex) : "";
}

// node_modules/mdast-util-to-hast/lib/handlers/text.js
function text4(state2, node2) {
  const result = { type: "text", value: trimLines(String(node2.value)) };
  state2.patch(node2, result);
  return state2.applyData(node2, result);
}

// node_modules/mdast-util-to-hast/lib/handlers/thematic-break.js
function thematicBreak2(state2, node2) {
  const result = {
    type: "element",
    tagName: "hr",
    properties: {},
    children: []
  };
  state2.patch(node2, result);
  return state2.applyData(node2, result);
}

// node_modules/mdast-util-to-hast/lib/handlers/index.js
var handlers = {
  blockquote,
  break: hardBreak,
  code,
  delete: strikethrough,
  emphasis,
  footnoteReference,
  heading,
  html: html3,
  imageReference,
  image,
  inlineCode,
  linkReference,
  link,
  listItem,
  list: list2,
  paragraph,
  // @ts-expect-error: root is different, but hard to type.
  root: root2,
  strong,
  table,
  tableCell,
  tableRow,
  text: text4,
  thematicBreak: thematicBreak2,
  toml: ignore,
  yaml: ignore,
  definition: ignore,
  footnoteDefinition: ignore
};
function ignore() {
  return void 0;
}

// node_modules/@ungap/structured-clone/esm/types.js
var VOID = -1;
var PRIMITIVE = 0;
var ARRAY = 1;
var OBJECT = 2;
var DATE = 3;
var REGEXP = 4;
var MAP = 5;
var SET = 6;
var ERROR = 7;
var BIGINT = 8;

// node_modules/@ungap/structured-clone/esm/deserialize.js
var { defineProperty } = Object;
var env = typeof self === "object" ? self : globalThis;
var guard = (name2, init) => {
  switch (name2) {
    case "Function":
    case "SharedWorker":
    case "Worker":
    case "eval":
    case "setInterval":
    case "setTimeout":
      throw new TypeError("unable to deserialize " + name2);
  }
  return new env[name2](init);
};
var deserializer = ($, _) => {
  const as = (out, index3) => {
    $.set(index3, out);
    return out;
  };
  const unpair = (index3) => {
    if ($.has(index3))
      return $.get(index3);
    const [type, value] = _[index3];
    switch (type) {
      case PRIMITIVE:
      case VOID:
        return as(value, index3);
      case ARRAY: {
        const arr = as([], index3);
        for (const index4 of value)
          arr.push(unpair(index4));
        return arr;
      }
      case OBJECT: {
        const object = as({}, index3);
        for (const [key, index4] of value) {
          const k = unpair(key), value2 = unpair(index4);
          if (k === "__proto__") defineProperty(object, k, {
            value: value2,
            configurable: true,
            enumerable: true,
            writable: true
          });
          else object[k] = value2;
        }
        return object;
      }
      case DATE:
        return as(new Date(value), index3);
      case REGEXP: {
        const { source, flags } = value;
        return as(new RegExp(source, flags), index3);
      }
      case MAP: {
        const map3 = as(/* @__PURE__ */ new Map(), index3);
        for (const [key, index4] of value)
          map3.set(unpair(key), unpair(index4));
        return map3;
      }
      case SET: {
        const set = as(/* @__PURE__ */ new Set(), index3);
        for (const index4 of value)
          set.add(unpair(index4));
        return set;
      }
      case ERROR: {
        const { name: name2, message } = value;
        return as(
          typeof env[name2] === "function" ? guard(name2, message) : new Error(message),
          index3
        );
      }
      case BIGINT:
        return as(BigInt(value), index3);
      case "BigInt":
        return as(Object(BigInt(value)), index3);
      case "ArrayBuffer":
        return as(new Uint8Array(value).buffer, value);
      case "DataView": {
        const { buffer } = new Uint8Array(value);
        return as(new DataView(buffer), value);
      }
      case "-0":
        return -0;
    }
    return as(guard(type, value), index3);
  };
  return unpair;
};
var deserialize = (serialized) => deserializer(/* @__PURE__ */ new Map(), serialized)(0);

// node_modules/@ungap/structured-clone/esm/serialize.js
var EMPTY = "";
var { toString: toString2 } = {};
var { keys, is } = Object;
var typeOf = (value) => {
  const type = typeof value;
  if (type !== "object" || !value)
    return [PRIMITIVE, type];
  const asString2 = toString2.call(value).slice(8, -1);
  switch (asString2) {
    case "Array":
      return [ARRAY, EMPTY];
    case "Object":
      return [OBJECT, EMPTY];
    case "Date":
      return [DATE, EMPTY];
    case "RegExp":
      return [REGEXP, EMPTY];
    case "Map":
      return [MAP, EMPTY];
    case "Set":
      return [SET, EMPTY];
    case "DataView":
      return [ARRAY, asString2];
  }
  if (asString2.includes("Array"))
    return [ARRAY, asString2];
  if (value instanceof Error)
    return [ERROR, value.name || "Error"];
  return [OBJECT, asString2];
};
var shouldSkip = ([TYPE, type]) => TYPE === PRIMITIVE && (type === "function" || type === "symbol");
var serializer = (strict, json, $, _) => {
  const as = (out, value) => {
    const index3 = _.push(out) - 1;
    $.set(value, index3);
    return index3;
  };
  const pair2 = (value) => {
    if ($.has(value))
      return $.get(value);
    let [TYPE, type] = typeOf(value);
    switch (TYPE) {
      case PRIMITIVE: {
        let entry = value;
        switch (type) {
          case "bigint":
            TYPE = BIGINT;
            entry = value.toString();
            break;
          case "number":
            if (!value && is(value, -0))
              return _.push(["-0"]) - 1;
            break;
          case "function":
          case "symbol":
            if (strict)
              throw new TypeError("unable to serialize " + type);
            entry = null;
            break;
          case "undefined":
            return as([VOID], value);
        }
        return as([TYPE, entry], value);
      }
      case ARRAY: {
        if (type) {
          let spread = value;
          if (type === "DataView") {
            spread = new Uint8Array(value.buffer);
          } else if (type === "ArrayBuffer") {
            spread = new Uint8Array(value);
          }
          return as([type, [...spread]], value);
        }
        const arr = [];
        const index3 = as([TYPE, arr], value);
        for (const entry of value)
          arr.push(pair2(entry));
        return index3;
      }
      case OBJECT: {
        if (type) {
          switch (type) {
            case "BigInt":
              return as([type, value.toString()], value);
            case "Boolean":
            case "Number":
            case "String":
              return as([type, value.valueOf()], value);
          }
        }
        if (json && "toJSON" in value)
          return pair2(value.toJSON());
        const entries = [];
        const index3 = as([TYPE, entries], value);
        for (const key of keys(value)) {
          if (strict || !shouldSkip(typeOf(value[key])))
            entries.push([pair2(key), pair2(value[key])]);
        }
        return index3;
      }
      case DATE:
        return as([TYPE, isNaN(value.getTime()) ? EMPTY : value.toISOString()], value);
      case REGEXP: {
        const { source, flags } = value;
        return as([TYPE, { source, flags }], value);
      }
      case MAP: {
        const entries = [];
        const index3 = as([TYPE, entries], value);
        for (const [key, entry] of value) {
          if (strict || !(shouldSkip(typeOf(key)) || shouldSkip(typeOf(entry))))
            entries.push([pair2(key), pair2(entry)]);
        }
        return index3;
      }
      case SET: {
        const entries = [];
        const index3 = as([TYPE, entries], value);
        for (const entry of value) {
          if (strict || !shouldSkip(typeOf(entry)))
            entries.push(pair2(entry));
        }
        return index3;
      }
    }
    const { message } = value;
    return as([TYPE, { name: type, message }], value);
  };
  return pair2;
};
var serialize = (value, { json, lossy } = {}) => {
  const _ = [];
  return serializer(!(json || lossy), !!json, /* @__PURE__ */ new Map(), _)(value), _;
};

// node_modules/@ungap/structured-clone/esm/index.js
var esm_default = typeof structuredClone === "function" ? (
  /* c8 ignore start */
  (any, options) => options && ("json" in options || "lossy" in options) ? deserialize(serialize(any, options)) : structuredClone(any)
) : (any, options) => deserialize(serialize(any, options));

// node_modules/mdast-util-to-hast/lib/footer.js
function defaultFootnoteBackContent(_, rereferenceIndex) {
  const result = [{ type: "text", value: "\u21A9" }];
  if (rereferenceIndex > 1) {
    result.push({
      type: "element",
      tagName: "sup",
      properties: {},
      children: [{ type: "text", value: String(rereferenceIndex) }]
    });
  }
  return result;
}
function defaultFootnoteBackLabel(referenceIndex, rereferenceIndex) {
  return "Back to reference " + (referenceIndex + 1) + (rereferenceIndex > 1 ? "-" + rereferenceIndex : "");
}
function footer(state2) {
  const clobberPrefix = typeof state2.options.clobberPrefix === "string" ? state2.options.clobberPrefix : "user-content-";
  const footnoteBackContent = state2.options.footnoteBackContent || defaultFootnoteBackContent;
  const footnoteBackLabel = state2.options.footnoteBackLabel || defaultFootnoteBackLabel;
  const footnoteLabel = state2.options.footnoteLabel || "Footnotes";
  const footnoteLabelTagName = state2.options.footnoteLabelTagName || "h2";
  const footnoteLabelProperties = state2.options.footnoteLabelProperties || {
    className: ["sr-only"]
  };
  const listItems = [];
  let referenceIndex = -1;
  while (++referenceIndex < state2.footnoteOrder.length) {
    const definition3 = state2.footnoteById.get(
      state2.footnoteOrder[referenceIndex]
    );
    if (!definition3) {
      continue;
    }
    const content3 = state2.all(definition3);
    const id = String(definition3.identifier).toUpperCase();
    const safeId = normalizeUri(id.toLowerCase());
    let rereferenceIndex = 0;
    const backReferences = [];
    const counts = state2.footnoteCounts.get(id);
    while (counts !== void 0 && ++rereferenceIndex <= counts) {
      if (backReferences.length > 0) {
        backReferences.push({ type: "text", value: " " });
      }
      let children = typeof footnoteBackContent === "string" ? footnoteBackContent : footnoteBackContent(referenceIndex, rereferenceIndex);
      if (typeof children === "string") {
        children = { type: "text", value: children };
      }
      backReferences.push({
        type: "element",
        tagName: "a",
        properties: {
          href: "#" + clobberPrefix + "fnref-" + safeId + (rereferenceIndex > 1 ? "-" + rereferenceIndex : ""),
          dataFootnoteBackref: "",
          ariaLabel: typeof footnoteBackLabel === "string" ? footnoteBackLabel : footnoteBackLabel(referenceIndex, rereferenceIndex),
          className: ["data-footnote-backref"]
        },
        children: Array.isArray(children) ? children : [children]
      });
    }
    const tail = content3[content3.length - 1];
    if (tail && tail.type === "element" && tail.tagName === "p") {
      const tailTail = tail.children[tail.children.length - 1];
      if (tailTail && tailTail.type === "text") {
        tailTail.value += " ";
      } else {
        tail.children.push({ type: "text", value: " " });
      }
      tail.children.push(...backReferences);
    } else {
      content3.push(...backReferences);
    }
    const listItem3 = {
      type: "element",
      tagName: "li",
      properties: { id: clobberPrefix + "fn-" + safeId },
      children: state2.wrap(content3, true)
    };
    state2.patch(definition3, listItem3);
    listItems.push(listItem3);
  }
  if (listItems.length === 0) {
    return;
  }
  return {
    type: "element",
    tagName: "section",
    properties: { dataFootnotes: true, className: ["footnotes"] },
    children: [
      {
        type: "element",
        tagName: footnoteLabelTagName,
        properties: {
          ...esm_default(footnoteLabelProperties),
          id: "footnote-label"
        },
        children: [{ type: "text", value: footnoteLabel }]
      },
      { type: "text", value: "\n" },
      {
        type: "element",
        tagName: "ol",
        properties: {},
        children: state2.wrap(listItems, true)
      },
      { type: "text", value: "\n" }
    ]
  };
}

// node_modules/unist-util-is/lib/index.js
var convert = (
  // Note: overloads in JSDoc can’t yet use different `@template`s.
  /**
   * @type {(
   *   (<Condition extends string>(test: Condition) => (node: unknown, index?: number | null | undefined, parent?: Parent | null | undefined, context?: unknown) => node is Node & {type: Condition}) &
   *   (<Condition extends Props>(test: Condition) => (node: unknown, index?: number | null | undefined, parent?: Parent | null | undefined, context?: unknown) => node is Node & Condition) &
   *   (<Condition extends TestFunction>(test: Condition) => (node: unknown, index?: number | null | undefined, parent?: Parent | null | undefined, context?: unknown) => node is Node & Predicate<Condition, Node>) &
   *   ((test?: null | undefined) => (node?: unknown, index?: number | null | undefined, parent?: Parent | null | undefined, context?: unknown) => node is Node) &
   *   ((test?: Test) => Check)
   * )}
   */
  /**
   * @param {Test} [test]
   * @returns {Check}
   */
  (function(test) {
    if (test === null || test === void 0) {
      return ok2;
    }
    if (typeof test === "function") {
      return castFactory(test);
    }
    if (typeof test === "object") {
      return Array.isArray(test) ? anyFactory(test) : (
        // Cast because `ReadonlyArray` goes into the above but `isArray`
        // narrows to `Array`.
        propertiesFactory(
          /** @type {Props} */
          test
        )
      );
    }
    if (typeof test === "string") {
      return typeFactory(test);
    }
    throw new Error("Expected function, string, or object as test");
  })
);
function anyFactory(tests) {
  const checks2 = [];
  let index3 = -1;
  while (++index3 < tests.length) {
    checks2[index3] = convert(tests[index3]);
  }
  return castFactory(any);
  function any(...parameters) {
    let index4 = -1;
    while (++index4 < checks2.length) {
      if (checks2[index4].apply(this, parameters)) return true;
    }
    return false;
  }
}
function propertiesFactory(check2) {
  const checkAsRecord = (
    /** @type {Record<string, unknown>} */
    check2
  );
  return castFactory(all2);
  function all2(node2) {
    const nodeAsRecord = (
      /** @type {Record<string, unknown>} */
      /** @type {unknown} */
      node2
    );
    let key;
    for (key in check2) {
      if (nodeAsRecord[key] !== checkAsRecord[key]) return false;
    }
    return true;
  }
}
function typeFactory(check2) {
  return castFactory(type);
  function type(node2) {
    return node2 && node2.type === check2;
  }
}
function castFactory(testFunction) {
  return check2;
  function check2(value, index3, parent) {
    return Boolean(
      looksLikeANode(value) && testFunction.call(
        this,
        value,
        typeof index3 === "number" ? index3 : void 0,
        parent || void 0
      )
    );
  }
}
function ok2() {
  return true;
}
function looksLikeANode(value) {
  return value !== null && typeof value === "object" && "type" in value;
}

// node_modules/unist-util-visit-parents/lib/color.js
function color(d) {
  return d;
}

// node_modules/unist-util-visit-parents/lib/index.js
var empty2 = [];
var CONTINUE = true;
var EXIT = false;
var SKIP = "skip";
function visitParents(tree, testOrEnterOrVisitors, enterOrVisitorsOrReverse, reverse) {
  if (!tree || typeof tree !== "object") {
    throw new Error("Unexpected value `" + tree + "` for `tree`, expected node");
  }
  let preorder = true;
  let check2;
  let visitors;
  let enter;
  let exit3;
  if (typeof enterOrVisitorsOrReverse === "function" || enterOrVisitorsOrReverse && typeof enterOrVisitorsOrReverse === "object") {
    check2 = /** @type {Test} */
    testOrEnterOrVisitors;
    visitors = enterOrVisitorsOrReverse;
    if (reverse) preorder = false;
  } else {
    visitors = /** @type {Visitors | Visitor} */
    testOrEnterOrVisitors;
    if (enterOrVisitorsOrReverse) preorder = false;
  }
  if (typeof visitors === "function") {
    enter = visitors;
  } else if (visitors && typeof visitors === "object") {
    if (visitors.enter === null || visitors.enter === void 0) {
    } else if (typeof visitors.enter === "function") {
      enter = visitors.enter;
    } else {
      throw new Error(
        "Unexpected value `" + visitors.enter + "` for `visitors.enter`, expected optional visitor"
      );
    }
    if (typeof visitors.exit === "function") {
      exit3 = visitors.exit;
    } else {
      throw new Error(
        "Unexpected value `" + visitors.exit + "` for `visitors.exit`, expected visitor (pass a visitor directly if you only want an enter visitor)"
      );
    }
  } else {
    throw new Error(
      "Unexpected value `" + visitors + "` for `enter` or `visitors`, expected visitor or object with visitors"
    );
  }
  const is3 = convert(check2);
  const step = preorder ? 1 : -1;
  factory(tree, void 0, [])();
  function factory(node2, index3, parents) {
    const value = (
      /** @type {Record<string, unknown>} */
      node2 && typeof node2 === "object" ? node2 : {}
    );
    if (typeof value.type === "string") {
      const name2 = (
        // `hast`
        typeof value.tagName === "string" ? value.tagName : (
          // `xast`
          typeof value.name === "string" ? value.name : void 0
        )
      );
      Object.defineProperty(visit2, "name", {
        value: "node (" + color(node2.type + (name2 ? "<" + name2 + ">" : "")) + ")"
      });
    }
    return visit2;
    function visit2() {
      const matches = !check2 || is3(node2, index3, parents[parents.length - 1] || void 0);
      let result = empty2;
      if (enter && matches) {
        result = toResult(enter(node2, parents, index3));
        if (result[0] === EXIT) {
          return result;
        }
      }
      if ("children" in node2 && node2.children) {
        const nodeAsParent = (
          /** @type {UnistParent} */
          node2
        );
        if (nodeAsParent.children && result[0] !== SKIP) {
          let offset = (preorder ? -1 : nodeAsParent.children.length) + step;
          const grandparents = [...parents, nodeAsParent];
          while (offset > -1 && offset < nodeAsParent.children.length) {
            const child = nodeAsParent.children[offset];
            const subresult = factory(child, offset, grandparents)();
            if (subresult[0] === EXIT) {
              return subresult;
            }
            offset = typeof subresult[1] === "number" ? subresult[1] : offset + step;
          }
        }
      }
      if (exit3 && matches) {
        const exitResult = toResult(exit3(node2, parents, index3));
        if (exitResult[0] === SKIP) {
          throw new Error(
            "Unexpected value `" + exitResult[0] + "` from `exit`, expected `CONTINUE`, `EXIT`, index, or nothing"
          );
        }
        if (exitResult[0] === EXIT || typeof exitResult[1] === "number") {
          return exitResult;
        }
      }
      return result;
    }
  }
}
function toResult(value) {
  if (Array.isArray(value)) {
    return value;
  }
  if (typeof value === "number") {
    return [CONTINUE, value];
  }
  return value === null || value === void 0 ? empty2 : [value];
}

// node_modules/unist-util-visit/lib/index.js
function visit(tree, testOrEnterOrVisitors, enterOrVisitorsOrReverse, maybeReverse) {
  let check2;
  let visitors;
  let reverse;
  if (typeof enterOrVisitorsOrReverse === "function" || enterOrVisitorsOrReverse && typeof enterOrVisitorsOrReverse === "object") {
    check2 = /** @type {Test} */
    testOrEnterOrVisitors;
    visitors = enterOrVisitorsOrReverse;
    reverse = maybeReverse;
  } else {
    visitors = /** @type {Visitor | Visitors} */
    testOrEnterOrVisitors;
    reverse = enterOrVisitorsOrReverse;
  }
  if (typeof visitors === "function") {
    visitParents(tree, check2, wrap(visitors), reverse);
  } else {
    visitParents(tree, check2, wrapVisitors(visitors), reverse);
  }
}
function wrapVisitors(visitors) {
  if (!visitors || typeof visitors !== "object") return visitors;
  return {
    enter: typeof visitors.enter === "function" ? wrap(visitors.enter) : visitors.enter,
    exit: typeof visitors.exit === "function" ? wrap(visitors.exit) : visitors.exit
  };
}
function wrap(visitor) {
  return overload;
  function overload(node2, parents, index3) {
    return visitor(node2, index3, parents[parents.length - 1]);
  }
}

// node_modules/mdast-util-to-hast/lib/state.js
var own3 = {}.hasOwnProperty;
var emptyOptions3 = {};
function createState(tree, options) {
  const settings = options || emptyOptions3;
  const definitionById = /* @__PURE__ */ new Map();
  const footnoteById = /* @__PURE__ */ new Map();
  const footnoteCounts = /* @__PURE__ */ new Map();
  const handlers2 = { ...handlers, ...settings.handlers };
  const state2 = {
    all: all2,
    applyData,
    definitionById,
    footnoteById,
    footnoteCounts,
    footnoteOrder: [],
    handlers: handlers2,
    one: one3,
    options: settings,
    patch,
    wrap: wrap2
  };
  visit(tree, function(node2) {
    if (node2.type === "definition" || node2.type === "footnoteDefinition") {
      const map3 = node2.type === "definition" ? definitionById : footnoteById;
      const id = String(node2.identifier).toUpperCase();
      if (!map3.has(id)) {
        map3.set(id, node2);
      }
    }
  });
  return state2;
  function one3(node2, parent) {
    const type = node2.type;
    const handle2 = state2.handlers[type];
    if (own3.call(state2.handlers, type) && handle2) {
      return handle2(state2, node2, parent);
    }
    if (state2.options.passThrough && state2.options.passThrough.includes(type)) {
      if ("children" in node2) {
        const { children, ...shallow } = node2;
        const result = esm_default(shallow);
        result.children = state2.all(node2);
        return result;
      }
      return esm_default(node2);
    }
    const unknown = state2.options.unknownHandler || defaultUnknownHandler;
    return unknown(state2, node2, parent);
  }
  function all2(parent) {
    const values = [];
    if ("children" in parent) {
      const nodes = parent.children;
      let index3 = -1;
      while (++index3 < nodes.length) {
        const result = state2.one(nodes[index3], parent);
        if (result) {
          if (index3 && nodes[index3 - 1].type === "break") {
            if (!Array.isArray(result) && result.type === "text") {
              result.value = trimMarkdownSpaceStart(result.value);
            }
            if (!Array.isArray(result) && result.type === "element") {
              const head = result.children[0];
              if (head && head.type === "text") {
                head.value = trimMarkdownSpaceStart(head.value);
              }
            }
          }
          if (Array.isArray(result)) {
            values.push(...result);
          } else {
            values.push(result);
          }
        }
      }
    }
    return values;
  }
}
function patch(from, to) {
  if (from.position) to.position = position(from);
}
function applyData(from, to) {
  let result = to;
  if (from && from.data) {
    const hName = from.data.hName;
    const hChildren = from.data.hChildren;
    const hProperties = from.data.hProperties;
    if (typeof hName === "string") {
      if (result.type === "element") {
        result.tagName = hName;
      } else {
        const children = "children" in result ? result.children : [result];
        result = { type: "element", tagName: hName, properties: {}, children };
      }
    }
    if (result.type === "element" && hProperties) {
      Object.assign(result.properties, esm_default(hProperties));
    }
    if ("children" in result && result.children && hChildren !== null && hChildren !== void 0) {
      result.children = hChildren;
    }
  }
  return result;
}
function defaultUnknownHandler(state2, node2) {
  const data = node2.data || {};
  const result = "value" in node2 && !(own3.call(data, "hProperties") || own3.call(data, "hChildren")) ? { type: "text", value: node2.value } : {
    type: "element",
    tagName: "div",
    properties: {},
    children: state2.all(node2)
  };
  state2.patch(node2, result);
  return state2.applyData(node2, result);
}
function wrap2(nodes, loose) {
  const result = [];
  let index3 = -1;
  if (loose) {
    result.push({ type: "text", value: "\n" });
  }
  while (++index3 < nodes.length) {
    if (index3) result.push({ type: "text", value: "\n" });
    result.push(nodes[index3]);
  }
  if (loose && nodes.length > 0) {
    result.push({ type: "text", value: "\n" });
  }
  return result;
}
function trimMarkdownSpaceStart(value) {
  let index3 = 0;
  let code4 = value.charCodeAt(index3);
  while (code4 === 9 || code4 === 32) {
    index3++;
    code4 = value.charCodeAt(index3);
  }
  return value.slice(index3);
}

// node_modules/mdast-util-to-hast/lib/index.js
function toHast(tree, options) {
  const state2 = createState(tree, options);
  const node2 = state2.one(tree, void 0);
  const foot = footer(state2);
  const result = Array.isArray(node2) ? { type: "root", children: node2 } : node2 || { type: "root", children: [] };
  if (foot) {
    ok("children" in result);
    result.children.push({ type: "text", value: "\n" }, foot);
  }
  return result;
}

// node_modules/remark-rehype/lib/index.js
function remarkRehype(destination, options) {
  if (destination && "run" in destination) {
    return async function(tree, file) {
      const hastTree = (
        /** @type {HastRoot} */
        toHast(tree, { file, ...options })
      );
      await destination.run(hastTree, file);
    };
  }
  return function(tree, file) {
    return (
      /** @type {HastRoot} */
      toHast(tree, { file, ...destination || options })
    );
  };
}

// node_modules/bail/index.js
function bail(error) {
  if (error) {
    throw error;
  }
}

// node_modules/unified/lib/index.js
var import_extend = __toESM(require_extend(), 1);

// node_modules/is-plain-obj/index.js
function isPlainObject(value) {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  const prototype = Object.getPrototypeOf(value);
  return (prototype === null || prototype === Object.prototype || Object.getPrototypeOf(prototype) === null) && !(Symbol.toStringTag in value) && !(Symbol.iterator in value);
}

// node_modules/trough/lib/index.js
function trough() {
  const fns = [];
  const pipeline = { run, use };
  return pipeline;
  function run(...values) {
    let middlewareIndex = -1;
    const callback = values.pop();
    if (typeof callback !== "function") {
      throw new TypeError("Expected function as last argument, not " + callback);
    }
    next(null, ...values);
    function next(error, ...output) {
      const fn = fns[++middlewareIndex];
      let index3 = -1;
      if (error) {
        callback(error);
        return;
      }
      while (++index3 < values.length) {
        if (output[index3] === null || output[index3] === void 0) {
          output[index3] = values[index3];
        }
      }
      values = output;
      if (fn) {
        wrap3(fn, next)(...output);
      } else {
        callback(null, ...output);
      }
    }
  }
  function use(middelware) {
    if (typeof middelware !== "function") {
      throw new TypeError(
        "Expected `middelware` to be a function, not " + middelware
      );
    }
    fns.push(middelware);
    return pipeline;
  }
}
function wrap3(middleware, callback) {
  let called;
  return wrapped;
  function wrapped(...parameters) {
    const fnExpectsCallback = middleware.length > parameters.length;
    let result;
    if (fnExpectsCallback) {
      parameters.push(done);
    }
    try {
      result = middleware.apply(this, parameters);
    } catch (error) {
      const exception = (
        /** @type {Error} */
        error
      );
      if (fnExpectsCallback && called) {
        throw exception;
      }
      return done(exception);
    }
    if (!fnExpectsCallback) {
      if (result && result.then && typeof result.then === "function") {
        result.then(then, done);
      } else if (result instanceof Error) {
        done(result);
      } else {
        then(result);
      }
    }
  }
  function done(error, ...output) {
    if (!called) {
      called = true;
      callback(error, ...output);
    }
  }
  function then(value) {
    done(null, value);
  }
}

// node_modules/vfile/lib/minpath.browser.js
var minpath = { basename, dirname, extname, join, sep: "/" };
function basename(path2, extname2) {
  if (extname2 !== void 0 && typeof extname2 !== "string") {
    throw new TypeError('"ext" argument must be a string');
  }
  assertPath(path2);
  let start2 = 0;
  let end = -1;
  let index3 = path2.length;
  let seenNonSlash;
  if (extname2 === void 0 || extname2.length === 0 || extname2.length > path2.length) {
    while (index3--) {
      if (path2.codePointAt(index3) === 47) {
        if (seenNonSlash) {
          start2 = index3 + 1;
          break;
        }
      } else if (end < 0) {
        seenNonSlash = true;
        end = index3 + 1;
      }
    }
    return end < 0 ? "" : path2.slice(start2, end);
  }
  if (extname2 === path2) {
    return "";
  }
  let firstNonSlashEnd = -1;
  let extnameIndex = extname2.length - 1;
  while (index3--) {
    if (path2.codePointAt(index3) === 47) {
      if (seenNonSlash) {
        start2 = index3 + 1;
        break;
      }
    } else {
      if (firstNonSlashEnd < 0) {
        seenNonSlash = true;
        firstNonSlashEnd = index3 + 1;
      }
      if (extnameIndex > -1) {
        if (path2.codePointAt(index3) === extname2.codePointAt(extnameIndex--)) {
          if (extnameIndex < 0) {
            end = index3;
          }
        } else {
          extnameIndex = -1;
          end = firstNonSlashEnd;
        }
      }
    }
  }
  if (start2 === end) {
    end = firstNonSlashEnd;
  } else if (end < 0) {
    end = path2.length;
  }
  return path2.slice(start2, end);
}
function dirname(path2) {
  assertPath(path2);
  if (path2.length === 0) {
    return ".";
  }
  let end = -1;
  let index3 = path2.length;
  let unmatchedSlash;
  while (--index3) {
    if (path2.codePointAt(index3) === 47) {
      if (unmatchedSlash) {
        end = index3;
        break;
      }
    } else if (!unmatchedSlash) {
      unmatchedSlash = true;
    }
  }
  return end < 0 ? path2.codePointAt(0) === 47 ? "/" : "." : end === 1 && path2.codePointAt(0) === 47 ? "//" : path2.slice(0, end);
}
function extname(path2) {
  assertPath(path2);
  let index3 = path2.length;
  let end = -1;
  let startPart = 0;
  let startDot = -1;
  let preDotState = 0;
  let unmatchedSlash;
  while (index3--) {
    const code4 = path2.codePointAt(index3);
    if (code4 === 47) {
      if (unmatchedSlash) {
        startPart = index3 + 1;
        break;
      }
      continue;
    }
    if (end < 0) {
      unmatchedSlash = true;
      end = index3 + 1;
    }
    if (code4 === 46) {
      if (startDot < 0) {
        startDot = index3;
      } else if (preDotState !== 1) {
        preDotState = 1;
      }
    } else if (startDot > -1) {
      preDotState = -1;
    }
  }
  if (startDot < 0 || end < 0 || // We saw a non-dot character immediately before the dot.
  preDotState === 0 || // The (right-most) trimmed path component is exactly `..`.
  preDotState === 1 && startDot === end - 1 && startDot === startPart + 1) {
    return "";
  }
  return path2.slice(startDot, end);
}
function join(...segments) {
  let index3 = -1;
  let joined;
  while (++index3 < segments.length) {
    assertPath(segments[index3]);
    if (segments[index3]) {
      joined = joined === void 0 ? segments[index3] : joined + "/" + segments[index3];
    }
  }
  return joined === void 0 ? "." : normalize2(joined);
}
function normalize2(path2) {
  assertPath(path2);
  const absolute = path2.codePointAt(0) === 47;
  let value = normalizeString(path2, !absolute);
  if (value.length === 0 && !absolute) {
    value = ".";
  }
  if (value.length > 0 && path2.codePointAt(path2.length - 1) === 47) {
    value += "/";
  }
  return absolute ? "/" + value : value;
}
function normalizeString(path2, allowAboveRoot) {
  let result = "";
  let lastSegmentLength = 0;
  let lastSlash = -1;
  let dots = 0;
  let index3 = -1;
  let code4;
  let lastSlashIndex;
  while (++index3 <= path2.length) {
    if (index3 < path2.length) {
      code4 = path2.codePointAt(index3);
    } else if (code4 === 47) {
      break;
    } else {
      code4 = 47;
    }
    if (code4 === 47) {
      if (lastSlash === index3 - 1 || dots === 1) {
      } else if (lastSlash !== index3 - 1 && dots === 2) {
        if (result.length < 2 || lastSegmentLength !== 2 || result.codePointAt(result.length - 1) !== 46 || result.codePointAt(result.length - 2) !== 46) {
          if (result.length > 2) {
            lastSlashIndex = result.lastIndexOf("/");
            if (lastSlashIndex !== result.length - 1) {
              if (lastSlashIndex < 0) {
                result = "";
                lastSegmentLength = 0;
              } else {
                result = result.slice(0, lastSlashIndex);
                lastSegmentLength = result.length - 1 - result.lastIndexOf("/");
              }
              lastSlash = index3;
              dots = 0;
              continue;
            }
          } else if (result.length > 0) {
            result = "";
            lastSegmentLength = 0;
            lastSlash = index3;
            dots = 0;
            continue;
          }
        }
        if (allowAboveRoot) {
          result = result.length > 0 ? result + "/.." : "..";
          lastSegmentLength = 2;
        }
      } else {
        if (result.length > 0) {
          result += "/" + path2.slice(lastSlash + 1, index3);
        } else {
          result = path2.slice(lastSlash + 1, index3);
        }
        lastSegmentLength = index3 - lastSlash - 1;
      }
      lastSlash = index3;
      dots = 0;
    } else if (code4 === 46 && dots > -1) {
      dots++;
    } else {
      dots = -1;
    }
  }
  return result;
}
function assertPath(path2) {
  if (typeof path2 !== "string") {
    throw new TypeError(
      "Path must be a string. Received " + JSON.stringify(path2)
    );
  }
}

// node_modules/vfile/lib/minproc.browser.js
var minproc = { cwd };
function cwd() {
  return "/";
}

// node_modules/vfile/lib/minurl.shared.js
function isUrl(fileUrlOrPath) {
  return Boolean(
    fileUrlOrPath !== null && typeof fileUrlOrPath === "object" && "href" in fileUrlOrPath && fileUrlOrPath.href && "protocol" in fileUrlOrPath && fileUrlOrPath.protocol && // @ts-expect-error: indexing is fine.
    fileUrlOrPath.auth === void 0
  );
}

// node_modules/vfile/lib/minurl.browser.js
function urlToPath(path2) {
  if (typeof path2 === "string") {
    path2 = new URL(path2);
  } else if (!isUrl(path2)) {
    const error = new TypeError(
      'The "path" argument must be of type string or an instance of URL. Received `' + path2 + "`"
    );
    error.code = "ERR_INVALID_ARG_TYPE";
    throw error;
  }
  if (path2.protocol !== "file:") {
    const error = new TypeError("The URL must be of scheme file");
    error.code = "ERR_INVALID_URL_SCHEME";
    throw error;
  }
  return getPathFromURLPosix(path2);
}
function getPathFromURLPosix(url) {
  if (url.hostname !== "") {
    const error = new TypeError(
      'File URL host must be "localhost" or empty on darwin'
    );
    error.code = "ERR_INVALID_FILE_URL_HOST";
    throw error;
  }
  const pathname = url.pathname;
  let index3 = -1;
  while (++index3 < pathname.length) {
    if (pathname.codePointAt(index3) === 37 && pathname.codePointAt(index3 + 1) === 50) {
      const third = pathname.codePointAt(index3 + 2);
      if (third === 70 || third === 102) {
        const error = new TypeError(
          "File URL path must not include encoded / characters"
        );
        error.code = "ERR_INVALID_FILE_URL_PATH";
        throw error;
      }
    }
  }
  return decodeURIComponent(pathname);
}

// node_modules/vfile/lib/index.js
var order = (
  /** @type {const} */
  [
    "history",
    "path",
    "basename",
    "stem",
    "extname",
    "dirname"
  ]
);
var VFile = class {
  /**
   * Create a new virtual file.
   *
   * `options` is treated as:
   *
   * *   `string` or `Uint8Array` — `{value: options}`
   * *   `URL` — `{path: options}`
   * *   `VFile` — shallow copies its data over to the new file
   * *   `object` — all fields are shallow copied over to the new file
   *
   * Path related fields are set in the following order (least specific to
   * most specific): `history`, `path`, `basename`, `stem`, `extname`,
   * `dirname`.
   *
   * You cannot set `dirname` or `extname` without setting either `history`,
   * `path`, `basename`, or `stem` too.
   *
   * @param {Compatible | null | undefined} [value]
   *   File value.
   * @returns
   *   New instance.
   */
  constructor(value) {
    let options;
    if (!value) {
      options = {};
    } else if (isUrl(value)) {
      options = { path: value };
    } else if (typeof value === "string" || isUint8Array(value)) {
      options = { value };
    } else {
      options = value;
    }
    this.cwd = "cwd" in options ? "" : minproc.cwd();
    this.data = {};
    this.history = [];
    this.messages = [];
    this.value;
    this.map;
    this.result;
    this.stored;
    let index3 = -1;
    while (++index3 < order.length) {
      const field2 = order[index3];
      if (field2 in options && options[field2] !== void 0 && options[field2] !== null) {
        this[field2] = field2 === "history" ? [...options[field2]] : options[field2];
      }
    }
    let field;
    for (field in options) {
      if (!order.includes(field)) {
        this[field] = options[field];
      }
    }
  }
  /**
   * Get the basename (including extname) (example: `'index.min.js'`).
   *
   * @returns {string | undefined}
   *   Basename.
   */
  get basename() {
    return typeof this.path === "string" ? minpath.basename(this.path) : void 0;
  }
  /**
   * Set basename (including extname) (`'index.min.js'`).
   *
   * Cannot contain path separators (`'/'` on unix, macOS, and browsers, `'\'`
   * on windows).
   * Cannot be nullified (use `file.path = file.dirname` instead).
   *
   * @param {string} basename
   *   Basename.
   * @returns {undefined}
   *   Nothing.
   */
  set basename(basename2) {
    assertNonEmpty(basename2, "basename");
    assertPart(basename2, "basename");
    this.path = minpath.join(this.dirname || "", basename2);
  }
  /**
   * Get the parent path (example: `'~'`).
   *
   * @returns {string | undefined}
   *   Dirname.
   */
  get dirname() {
    return typeof this.path === "string" ? minpath.dirname(this.path) : void 0;
  }
  /**
   * Set the parent path (example: `'~'`).
   *
   * Cannot be set if there’s no `path` yet.
   *
   * @param {string | undefined} dirname
   *   Dirname.
   * @returns {undefined}
   *   Nothing.
   */
  set dirname(dirname2) {
    assertPath2(this.basename, "dirname");
    this.path = minpath.join(dirname2 || "", this.basename);
  }
  /**
   * Get the extname (including dot) (example: `'.js'`).
   *
   * @returns {string | undefined}
   *   Extname.
   */
  get extname() {
    return typeof this.path === "string" ? minpath.extname(this.path) : void 0;
  }
  /**
   * Set the extname (including dot) (example: `'.js'`).
   *
   * Cannot contain path separators (`'/'` on unix, macOS, and browsers, `'\'`
   * on windows).
   * Cannot be set if there’s no `path` yet.
   *
   * @param {string | undefined} extname
   *   Extname.
   * @returns {undefined}
   *   Nothing.
   */
  set extname(extname2) {
    assertPart(extname2, "extname");
    assertPath2(this.dirname, "extname");
    if (extname2) {
      if (extname2.codePointAt(0) !== 46) {
        throw new Error("`extname` must start with `.`");
      }
      if (extname2.includes(".", 1)) {
        throw new Error("`extname` cannot contain multiple dots");
      }
    }
    this.path = minpath.join(this.dirname, this.stem + (extname2 || ""));
  }
  /**
   * Get the full path (example: `'~/index.min.js'`).
   *
   * @returns {string}
   *   Path.
   */
  get path() {
    return this.history[this.history.length - 1];
  }
  /**
   * Set the full path (example: `'~/index.min.js'`).
   *
   * Cannot be nullified.
   * You can set a file URL (a `URL` object with a `file:` protocol) which will
   * be turned into a path with `url.fileURLToPath`.
   *
   * @param {URL | string} path
   *   Path.
   * @returns {undefined}
   *   Nothing.
   */
  set path(path2) {
    if (isUrl(path2)) {
      path2 = urlToPath(path2);
    }
    assertNonEmpty(path2, "path");
    if (this.path !== path2) {
      this.history.push(path2);
    }
  }
  /**
   * Get the stem (basename w/o extname) (example: `'index.min'`).
   *
   * @returns {string | undefined}
   *   Stem.
   */
  get stem() {
    return typeof this.path === "string" ? minpath.basename(this.path, this.extname) : void 0;
  }
  /**
   * Set the stem (basename w/o extname) (example: `'index.min'`).
   *
   * Cannot contain path separators (`'/'` on unix, macOS, and browsers, `'\'`
   * on windows).
   * Cannot be nullified (use `file.path = file.dirname` instead).
   *
   * @param {string} stem
   *   Stem.
   * @returns {undefined}
   *   Nothing.
   */
  set stem(stem) {
    assertNonEmpty(stem, "stem");
    assertPart(stem, "stem");
    this.path = minpath.join(this.dirname || "", stem + (this.extname || ""));
  }
  // Normal prototypal methods.
  /**
   * Create a fatal message for `reason` associated with the file.
   *
   * The `fatal` field of the message is set to `true` (error; file not usable)
   * and the `file` field is set to the current file path.
   * The message is added to the `messages` field on `file`.
   *
   * > 🪦 **Note**: also has obsolete signatures.
   *
   * @overload
   * @param {string} reason
   * @param {MessageOptions | null | undefined} [options]
   * @returns {never}
   *
   * @overload
   * @param {string} reason
   * @param {Node | NodeLike | null | undefined} parent
   * @param {string | null | undefined} [origin]
   * @returns {never}
   *
   * @overload
   * @param {string} reason
   * @param {Point | Position | null | undefined} place
   * @param {string | null | undefined} [origin]
   * @returns {never}
   *
   * @overload
   * @param {string} reason
   * @param {string | null | undefined} [origin]
   * @returns {never}
   *
   * @overload
   * @param {Error | VFileMessage} cause
   * @param {Node | NodeLike | null | undefined} parent
   * @param {string | null | undefined} [origin]
   * @returns {never}
   *
   * @overload
   * @param {Error | VFileMessage} cause
   * @param {Point | Position | null | undefined} place
   * @param {string | null | undefined} [origin]
   * @returns {never}
   *
   * @overload
   * @param {Error | VFileMessage} cause
   * @param {string | null | undefined} [origin]
   * @returns {never}
   *
   * @param {Error | VFileMessage | string} causeOrReason
   *   Reason for message, should use markdown.
   * @param {Node | NodeLike | MessageOptions | Point | Position | string | null | undefined} [optionsOrParentOrPlace]
   *   Configuration (optional).
   * @param {string | null | undefined} [origin]
   *   Place in code where the message originates (example:
   *   `'my-package:my-rule'` or `'my-rule'`).
   * @returns {never}
   *   Never.
   * @throws {VFileMessage}
   *   Message.
   */
  fail(causeOrReason, optionsOrParentOrPlace, origin) {
    const message = this.message(causeOrReason, optionsOrParentOrPlace, origin);
    message.fatal = true;
    throw message;
  }
  /**
   * Create an info message for `reason` associated with the file.
   *
   * The `fatal` field of the message is set to `undefined` (info; change
   * likely not needed) and the `file` field is set to the current file path.
   * The message is added to the `messages` field on `file`.
   *
   * > 🪦 **Note**: also has obsolete signatures.
   *
   * @overload
   * @param {string} reason
   * @param {MessageOptions | null | undefined} [options]
   * @returns {VFileMessage}
   *
   * @overload
   * @param {string} reason
   * @param {Node | NodeLike | null | undefined} parent
   * @param {string | null | undefined} [origin]
   * @returns {VFileMessage}
   *
   * @overload
   * @param {string} reason
   * @param {Point | Position | null | undefined} place
   * @param {string | null | undefined} [origin]
   * @returns {VFileMessage}
   *
   * @overload
   * @param {string} reason
   * @param {string | null | undefined} [origin]
   * @returns {VFileMessage}
   *
   * @overload
   * @param {Error | VFileMessage} cause
   * @param {Node | NodeLike | null | undefined} parent
   * @param {string | null | undefined} [origin]
   * @returns {VFileMessage}
   *
   * @overload
   * @param {Error | VFileMessage} cause
   * @param {Point | Position | null | undefined} place
   * @param {string | null | undefined} [origin]
   * @returns {VFileMessage}
   *
   * @overload
   * @param {Error | VFileMessage} cause
   * @param {string | null | undefined} [origin]
   * @returns {VFileMessage}
   *
   * @param {Error | VFileMessage | string} causeOrReason
   *   Reason for message, should use markdown.
   * @param {Node | NodeLike | MessageOptions | Point | Position | string | null | undefined} [optionsOrParentOrPlace]
   *   Configuration (optional).
   * @param {string | null | undefined} [origin]
   *   Place in code where the message originates (example:
   *   `'my-package:my-rule'` or `'my-rule'`).
   * @returns {VFileMessage}
   *   Message.
   */
  info(causeOrReason, optionsOrParentOrPlace, origin) {
    const message = this.message(causeOrReason, optionsOrParentOrPlace, origin);
    message.fatal = void 0;
    return message;
  }
  /**
   * Create a message for `reason` associated with the file.
   *
   * The `fatal` field of the message is set to `false` (warning; change may be
   * needed) and the `file` field is set to the current file path.
   * The message is added to the `messages` field on `file`.
   *
   * > 🪦 **Note**: also has obsolete signatures.
   *
   * @overload
   * @param {string} reason
   * @param {MessageOptions | null | undefined} [options]
   * @returns {VFileMessage}
   *
   * @overload
   * @param {string} reason
   * @param {Node | NodeLike | null | undefined} parent
   * @param {string | null | undefined} [origin]
   * @returns {VFileMessage}
   *
   * @overload
   * @param {string} reason
   * @param {Point | Position | null | undefined} place
   * @param {string | null | undefined} [origin]
   * @returns {VFileMessage}
   *
   * @overload
   * @param {string} reason
   * @param {string | null | undefined} [origin]
   * @returns {VFileMessage}
   *
   * @overload
   * @param {Error | VFileMessage} cause
   * @param {Node | NodeLike | null | undefined} parent
   * @param {string | null | undefined} [origin]
   * @returns {VFileMessage}
   *
   * @overload
   * @param {Error | VFileMessage} cause
   * @param {Point | Position | null | undefined} place
   * @param {string | null | undefined} [origin]
   * @returns {VFileMessage}
   *
   * @overload
   * @param {Error | VFileMessage} cause
   * @param {string | null | undefined} [origin]
   * @returns {VFileMessage}
   *
   * @param {Error | VFileMessage | string} causeOrReason
   *   Reason for message, should use markdown.
   * @param {Node | NodeLike | MessageOptions | Point | Position | string | null | undefined} [optionsOrParentOrPlace]
   *   Configuration (optional).
   * @param {string | null | undefined} [origin]
   *   Place in code where the message originates (example:
   *   `'my-package:my-rule'` or `'my-rule'`).
   * @returns {VFileMessage}
   *   Message.
   */
  message(causeOrReason, optionsOrParentOrPlace, origin) {
    const message = new VFileMessage(
      // @ts-expect-error: the overloads are fine.
      causeOrReason,
      optionsOrParentOrPlace,
      origin
    );
    if (this.path) {
      message.name = this.path + ":" + message.name;
      message.file = this.path;
    }
    message.fatal = false;
    this.messages.push(message);
    return message;
  }
  /**
   * Serialize the file.
   *
   * > **Note**: which encodings are supported depends on the engine.
   * > For info on Node.js, see:
   * > <https://nodejs.org/api/util.html#whatwg-supported-encodings>.
   *
   * @param {string | null | undefined} [encoding='utf8']
   *   Character encoding to understand `value` as when it’s a `Uint8Array`
   *   (default: `'utf-8'`).
   * @returns {string}
   *   Serialized file.
   */
  toString(encoding) {
    if (this.value === void 0) {
      return "";
    }
    if (typeof this.value === "string") {
      return this.value;
    }
    const decoder = new TextDecoder(encoding || void 0);
    return decoder.decode(this.value);
  }
};
function assertPart(part, name2) {
  if (part && part.includes(minpath.sep)) {
    throw new Error(
      "`" + name2 + "` cannot be a path: did not expect `" + minpath.sep + "`"
    );
  }
}
function assertNonEmpty(part, name2) {
  if (!part) {
    throw new Error("`" + name2 + "` cannot be empty");
  }
}
function assertPath2(path2, name2) {
  if (!path2) {
    throw new Error("Setting `" + name2 + "` requires `path` to be set too");
  }
}
function isUint8Array(value) {
  return Boolean(
    value && typeof value === "object" && "byteLength" in value && "byteOffset" in value
  );
}

// node_modules/unified/lib/callable-instance.js
var CallableInstance = (
  /**
   * @type {new <Parameters extends Array<unknown>, Result>(property: string | symbol) => (...parameters: Parameters) => Result}
   */
  /** @type {unknown} */
  /**
   * @this {Function}
   * @param {string | symbol} property
   * @returns {(...parameters: Array<unknown>) => unknown}
   */
  (function(property) {
    const self2 = this;
    const constr = self2.constructor;
    const proto = (
      /** @type {Record<string | symbol, Function>} */
      // Prototypes do exist.
      // type-coverage:ignore-next-line
      constr.prototype
    );
    const value = proto[property];
    const apply = function() {
      return value.apply(apply, arguments);
    };
    Object.setPrototypeOf(apply, proto);
    return apply;
  })
);

// node_modules/unified/lib/index.js
var own4 = {}.hasOwnProperty;
var Processor = class _Processor extends CallableInstance {
  /**
   * Create a processor.
   */
  constructor() {
    super("copy");
    this.Compiler = void 0;
    this.Parser = void 0;
    this.attachers = [];
    this.compiler = void 0;
    this.freezeIndex = -1;
    this.frozen = void 0;
    this.namespace = {};
    this.parser = void 0;
    this.transformers = trough();
  }
  /**
   * Copy a processor.
   *
   * @deprecated
   *   This is a private internal method and should not be used.
   * @returns {Processor<ParseTree, HeadTree, TailTree, CompileTree, CompileResult>}
   *   New *unfrozen* processor ({@linkcode Processor}) that is
   *   configured to work the same as its ancestor.
   *   When the descendant processor is configured in the future it does not
   *   affect the ancestral processor.
   */
  copy() {
    const destination = (
      /** @type {Processor<ParseTree, HeadTree, TailTree, CompileTree, CompileResult>} */
      new _Processor()
    );
    let index3 = -1;
    while (++index3 < this.attachers.length) {
      const attacher = this.attachers[index3];
      destination.use(...attacher);
    }
    destination.data((0, import_extend.default)(true, {}, this.namespace));
    return destination;
  }
  /**
   * Configure the processor with info available to all plugins.
   * Information is stored in an object.
   *
   * Typically, options can be given to a specific plugin, but sometimes it
   * makes sense to have information shared with several plugins.
   * For example, a list of HTML elements that are self-closing, which is
   * needed during all phases.
   *
   * > **Note**: setting information cannot occur on *frozen* processors.
   * > Call the processor first to create a new unfrozen processor.
   *
   * > **Note**: to register custom data in TypeScript, augment the
   * > {@linkcode Data} interface.
   *
   * @example
   *   This example show how to get and set info:
   *
   *   ```js
   *   import {unified} from 'unified'
   *
   *   const processor = unified().data('alpha', 'bravo')
   *
   *   processor.data('alpha') // => 'bravo'
   *
   *   processor.data() // => {alpha: 'bravo'}
   *
   *   processor.data({charlie: 'delta'})
   *
   *   processor.data() // => {charlie: 'delta'}
   *   ```
   *
   * @template {keyof Data} Key
   *
   * @overload
   * @returns {Data}
   *
   * @overload
   * @param {Data} dataset
   * @returns {Processor<ParseTree, HeadTree, TailTree, CompileTree, CompileResult>}
   *
   * @overload
   * @param {Key} key
   * @returns {Data[Key]}
   *
   * @overload
   * @param {Key} key
   * @param {Data[Key]} value
   * @returns {Processor<ParseTree, HeadTree, TailTree, CompileTree, CompileResult>}
   *
   * @param {Data | Key} [key]
   *   Key to get or set, or entire dataset to set, or nothing to get the
   *   entire dataset (optional).
   * @param {Data[Key]} [value]
   *   Value to set (optional).
   * @returns {unknown}
   *   The current processor when setting, the value at `key` when getting, or
   *   the entire dataset when getting without key.
   */
  data(key, value) {
    if (typeof key === "string") {
      if (arguments.length === 2) {
        assertUnfrozen("data", this.frozen);
        this.namespace[key] = value;
        return this;
      }
      return own4.call(this.namespace, key) && this.namespace[key] || void 0;
    }
    if (key) {
      assertUnfrozen("data", this.frozen);
      this.namespace = key;
      return this;
    }
    return this.namespace;
  }
  /**
   * Freeze a processor.
   *
   * Frozen processors are meant to be extended and not to be configured
   * directly.
   *
   * When a processor is frozen it cannot be unfrozen.
   * New processors working the same way can be created by calling the
   * processor.
   *
   * It’s possible to freeze processors explicitly by calling `.freeze()`.
   * Processors freeze automatically when `.parse()`, `.run()`, `.runSync()`,
   * `.stringify()`, `.process()`, or `.processSync()` are called.
   *
   * @returns {Processor<ParseTree, HeadTree, TailTree, CompileTree, CompileResult>}
   *   The current processor.
   */
  freeze() {
    if (this.frozen) {
      return this;
    }
    const self2 = (
      /** @type {Processor} */
      /** @type {unknown} */
      this
    );
    while (++this.freezeIndex < this.attachers.length) {
      const [attacher, ...options] = this.attachers[this.freezeIndex];
      if (options[0] === false) {
        continue;
      }
      if (options[0] === true) {
        options[0] = void 0;
      }
      const transformer = attacher.call(self2, ...options);
      if (typeof transformer === "function") {
        this.transformers.use(transformer);
      }
    }
    this.frozen = true;
    this.freezeIndex = Number.POSITIVE_INFINITY;
    return this;
  }
  /**
   * Parse text to a syntax tree.
   *
   * > **Note**: `parse` freezes the processor if not already *frozen*.
   *
   * > **Note**: `parse` performs the parse phase, not the run phase or other
   * > phases.
   *
   * @param {Compatible | undefined} [file]
   *   file to parse (optional); typically `string` or `VFile`; any value
   *   accepted as `x` in `new VFile(x)`.
   * @returns {ParseTree extends undefined ? Node : ParseTree}
   *   Syntax tree representing `file`.
   */
  parse(file) {
    this.freeze();
    const realFile = vfile(file);
    const parser = this.parser || this.Parser;
    assertParser("parse", parser);
    return parser(String(realFile), realFile);
  }
  /**
   * Process the given file as configured on the processor.
   *
   * > **Note**: `process` freezes the processor if not already *frozen*.
   *
   * > **Note**: `process` performs the parse, run, and stringify phases.
   *
   * @overload
   * @param {Compatible | undefined} file
   * @param {ProcessCallback<VFileWithOutput<CompileResult>>} done
   * @returns {undefined}
   *
   * @overload
   * @param {Compatible | undefined} [file]
   * @returns {Promise<VFileWithOutput<CompileResult>>}
   *
   * @param {Compatible | undefined} [file]
   *   File (optional); typically `string` or `VFile`]; any value accepted as
   *   `x` in `new VFile(x)`.
   * @param {ProcessCallback<VFileWithOutput<CompileResult>> | undefined} [done]
   *   Callback (optional).
   * @returns {Promise<VFile> | undefined}
   *   Nothing if `done` is given.
   *   Otherwise a promise, rejected with a fatal error or resolved with the
   *   processed file.
   *
   *   The parsed, transformed, and compiled value is available at
   *   `file.value` (see note).
   *
   *   > **Note**: unified typically compiles by serializing: most
   *   > compilers return `string` (or `Uint8Array`).
   *   > Some compilers, such as the one configured with
   *   > [`rehype-react`][rehype-react], return other values (in this case, a
   *   > React tree).
   *   > If you’re using a compiler that doesn’t serialize, expect different
   *   > result values.
   *   >
   *   > To register custom results in TypeScript, add them to
   *   > {@linkcode CompileResultMap}.
   *
   *   [rehype-react]: https://github.com/rehypejs/rehype-react
   */
  process(file, done) {
    const self2 = this;
    this.freeze();
    assertParser("process", this.parser || this.Parser);
    assertCompiler("process", this.compiler || this.Compiler);
    return done ? executor(void 0, done) : new Promise(executor);
    function executor(resolve, reject) {
      const realFile = vfile(file);
      const parseTree = (
        /** @type {HeadTree extends undefined ? Node : HeadTree} */
        /** @type {unknown} */
        self2.parse(realFile)
      );
      self2.run(parseTree, realFile, function(error, tree, file2) {
        if (error || !tree || !file2) {
          return realDone(error);
        }
        const compileTree = (
          /** @type {CompileTree extends undefined ? Node : CompileTree} */
          /** @type {unknown} */
          tree
        );
        const compileResult = self2.stringify(compileTree, file2);
        if (looksLikeAValue(compileResult)) {
          file2.value = compileResult;
        } else {
          file2.result = compileResult;
        }
        realDone(
          error,
          /** @type {VFileWithOutput<CompileResult>} */
          file2
        );
      });
      function realDone(error, file2) {
        if (error || !file2) {
          reject(error);
        } else if (resolve) {
          resolve(file2);
        } else {
          ok(done, "`done` is defined if `resolve` is not");
          done(void 0, file2);
        }
      }
    }
  }
  /**
   * Process the given file as configured on the processor.
   *
   * An error is thrown if asynchronous transforms are configured.
   *
   * > **Note**: `processSync` freezes the processor if not already *frozen*.
   *
   * > **Note**: `processSync` performs the parse, run, and stringify phases.
   *
   * @param {Compatible | undefined} [file]
   *   File (optional); typically `string` or `VFile`; any value accepted as
   *   `x` in `new VFile(x)`.
   * @returns {VFileWithOutput<CompileResult>}
   *   The processed file.
   *
   *   The parsed, transformed, and compiled value is available at
   *   `file.value` (see note).
   *
   *   > **Note**: unified typically compiles by serializing: most
   *   > compilers return `string` (or `Uint8Array`).
   *   > Some compilers, such as the one configured with
   *   > [`rehype-react`][rehype-react], return other values (in this case, a
   *   > React tree).
   *   > If you’re using a compiler that doesn’t serialize, expect different
   *   > result values.
   *   >
   *   > To register custom results in TypeScript, add them to
   *   > {@linkcode CompileResultMap}.
   *
   *   [rehype-react]: https://github.com/rehypejs/rehype-react
   */
  processSync(file) {
    let complete = false;
    let result;
    this.freeze();
    assertParser("processSync", this.parser || this.Parser);
    assertCompiler("processSync", this.compiler || this.Compiler);
    this.process(file, realDone);
    assertDone("processSync", "process", complete);
    ok(result, "we either bailed on an error or have a tree");
    return result;
    function realDone(error, file2) {
      complete = true;
      bail(error);
      result = file2;
    }
  }
  /**
   * Run *transformers* on a syntax tree.
   *
   * > **Note**: `run` freezes the processor if not already *frozen*.
   *
   * > **Note**: `run` performs the run phase, not other phases.
   *
   * @overload
   * @param {HeadTree extends undefined ? Node : HeadTree} tree
   * @param {RunCallback<TailTree extends undefined ? Node : TailTree>} done
   * @returns {undefined}
   *
   * @overload
   * @param {HeadTree extends undefined ? Node : HeadTree} tree
   * @param {Compatible | undefined} file
   * @param {RunCallback<TailTree extends undefined ? Node : TailTree>} done
   * @returns {undefined}
   *
   * @overload
   * @param {HeadTree extends undefined ? Node : HeadTree} tree
   * @param {Compatible | undefined} [file]
   * @returns {Promise<TailTree extends undefined ? Node : TailTree>}
   *
   * @param {HeadTree extends undefined ? Node : HeadTree} tree
   *   Tree to transform and inspect.
   * @param {(
   *   RunCallback<TailTree extends undefined ? Node : TailTree> |
   *   Compatible
   * )} [file]
   *   File associated with `node` (optional); any value accepted as `x` in
   *   `new VFile(x)`.
   * @param {RunCallback<TailTree extends undefined ? Node : TailTree>} [done]
   *   Callback (optional).
   * @returns {Promise<TailTree extends undefined ? Node : TailTree> | undefined}
   *   Nothing if `done` is given.
   *   Otherwise, a promise rejected with a fatal error or resolved with the
   *   transformed tree.
   */
  run(tree, file, done) {
    assertNode(tree);
    this.freeze();
    const transformers = this.transformers;
    if (!done && typeof file === "function") {
      done = file;
      file = void 0;
    }
    return done ? executor(void 0, done) : new Promise(executor);
    function executor(resolve, reject) {
      ok(
        typeof file !== "function",
        "`file` can\u2019t be a `done` anymore, we checked"
      );
      const realFile = vfile(file);
      transformers.run(tree, realFile, realDone);
      function realDone(error, outputTree, file2) {
        const resultingTree = (
          /** @type {TailTree extends undefined ? Node : TailTree} */
          outputTree || tree
        );
        if (error) {
          reject(error);
        } else if (resolve) {
          resolve(resultingTree);
        } else {
          ok(done, "`done` is defined if `resolve` is not");
          done(void 0, resultingTree, file2);
        }
      }
    }
  }
  /**
   * Run *transformers* on a syntax tree.
   *
   * An error is thrown if asynchronous transforms are configured.
   *
   * > **Note**: `runSync` freezes the processor if not already *frozen*.
   *
   * > **Note**: `runSync` performs the run phase, not other phases.
   *
   * @param {HeadTree extends undefined ? Node : HeadTree} tree
   *   Tree to transform and inspect.
   * @param {Compatible | undefined} [file]
   *   File associated with `node` (optional); any value accepted as `x` in
   *   `new VFile(x)`.
   * @returns {TailTree extends undefined ? Node : TailTree}
   *   Transformed tree.
   */
  runSync(tree, file) {
    let complete = false;
    let result;
    this.run(tree, file, realDone);
    assertDone("runSync", "run", complete);
    ok(result, "we either bailed on an error or have a tree");
    return result;
    function realDone(error, tree2) {
      bail(error);
      result = tree2;
      complete = true;
    }
  }
  /**
   * Compile a syntax tree.
   *
   * > **Note**: `stringify` freezes the processor if not already *frozen*.
   *
   * > **Note**: `stringify` performs the stringify phase, not the run phase
   * > or other phases.
   *
   * @param {CompileTree extends undefined ? Node : CompileTree} tree
   *   Tree to compile.
   * @param {Compatible | undefined} [file]
   *   File associated with `node` (optional); any value accepted as `x` in
   *   `new VFile(x)`.
   * @returns {CompileResult extends undefined ? Value : CompileResult}
   *   Textual representation of the tree (see note).
   *
   *   > **Note**: unified typically compiles by serializing: most compilers
   *   > return `string` (or `Uint8Array`).
   *   > Some compilers, such as the one configured with
   *   > [`rehype-react`][rehype-react], return other values (in this case, a
   *   > React tree).
   *   > If you’re using a compiler that doesn’t serialize, expect different
   *   > result values.
   *   >
   *   > To register custom results in TypeScript, add them to
   *   > {@linkcode CompileResultMap}.
   *
   *   [rehype-react]: https://github.com/rehypejs/rehype-react
   */
  stringify(tree, file) {
    this.freeze();
    const realFile = vfile(file);
    const compiler2 = this.compiler || this.Compiler;
    assertCompiler("stringify", compiler2);
    assertNode(tree);
    return compiler2(tree, realFile);
  }
  /**
   * Configure the processor to use a plugin, a list of usable values, or a
   * preset.
   *
   * If the processor is already using a plugin, the previous plugin
   * configuration is changed based on the options that are passed in.
   * In other words, the plugin is not added a second time.
   *
   * > **Note**: `use` cannot be called on *frozen* processors.
   * > Call the processor first to create a new unfrozen processor.
   *
   * @example
   *   There are many ways to pass plugins to `.use()`.
   *   This example gives an overview:
   *
   *   ```js
   *   import {unified} from 'unified'
   *
   *   unified()
   *     // Plugin with options:
   *     .use(pluginA, {x: true, y: true})
   *     // Passing the same plugin again merges configuration (to `{x: true, y: false, z: true}`):
   *     .use(pluginA, {y: false, z: true})
   *     // Plugins:
   *     .use([pluginB, pluginC])
   *     // Two plugins, the second with options:
   *     .use([pluginD, [pluginE, {}]])
   *     // Preset with plugins and settings:
   *     .use({plugins: [pluginF, [pluginG, {}]], settings: {position: false}})
   *     // Settings only:
   *     .use({settings: {position: false}})
   *   ```
   *
   * @template {Array<unknown>} [Parameters=[]]
   * @template {Node | string | undefined} [Input=undefined]
   * @template [Output=Input]
   *
   * @overload
   * @param {Preset | null | undefined} [preset]
   * @returns {Processor<ParseTree, HeadTree, TailTree, CompileTree, CompileResult>}
   *
   * @overload
   * @param {PluggableList} list
   * @returns {Processor<ParseTree, HeadTree, TailTree, CompileTree, CompileResult>}
   *
   * @overload
   * @param {Plugin<Parameters, Input, Output>} plugin
   * @param {...(Parameters | [boolean])} parameters
   * @returns {UsePlugin<ParseTree, HeadTree, TailTree, CompileTree, CompileResult, Input, Output>}
   *
   * @param {PluggableList | Plugin | Preset | null | undefined} value
   *   Usable value.
   * @param {...unknown} parameters
   *   Parameters, when a plugin is given as a usable value.
   * @returns {Processor<ParseTree, HeadTree, TailTree, CompileTree, CompileResult>}
   *   Current processor.
   */
  use(value, ...parameters) {
    const attachers = this.attachers;
    const namespace = this.namespace;
    assertUnfrozen("use", this.frozen);
    if (value === null || value === void 0) {
    } else if (typeof value === "function") {
      addPlugin(value, parameters);
    } else if (typeof value === "object") {
      if (Array.isArray(value)) {
        addList(value);
      } else {
        addPreset(value);
      }
    } else {
      throw new TypeError("Expected usable value, not `" + value + "`");
    }
    return this;
    function add(value2) {
      if (typeof value2 === "function") {
        addPlugin(value2, []);
      } else if (typeof value2 === "object") {
        if (Array.isArray(value2)) {
          const [plugin, ...parameters2] = (
            /** @type {PluginTuple<Array<unknown>>} */
            value2
          );
          addPlugin(plugin, parameters2);
        } else {
          addPreset(value2);
        }
      } else {
        throw new TypeError("Expected usable value, not `" + value2 + "`");
      }
    }
    function addPreset(result) {
      if (!("plugins" in result) && !("settings" in result)) {
        throw new Error(
          "Expected usable value but received an empty preset, which is probably a mistake: presets typically come with `plugins` and sometimes with `settings`, but this has neither"
        );
      }
      addList(result.plugins);
      if (result.settings) {
        namespace.settings = (0, import_extend.default)(true, namespace.settings, result.settings);
      }
    }
    function addList(plugins) {
      let index3 = -1;
      if (plugins === null || plugins === void 0) {
      } else if (Array.isArray(plugins)) {
        while (++index3 < plugins.length) {
          const thing = plugins[index3];
          add(thing);
        }
      } else {
        throw new TypeError("Expected a list of plugins, not `" + plugins + "`");
      }
    }
    function addPlugin(plugin, parameters2) {
      let index3 = -1;
      let entryIndex = -1;
      while (++index3 < attachers.length) {
        if (attachers[index3][0] === plugin) {
          entryIndex = index3;
          break;
        }
      }
      if (entryIndex === -1) {
        attachers.push([plugin, ...parameters2]);
      } else if (parameters2.length > 0) {
        let [primary, ...rest] = parameters2;
        const currentPrimary = attachers[entryIndex][1];
        if (isPlainObject(currentPrimary) && isPlainObject(primary)) {
          primary = (0, import_extend.default)(true, currentPrimary, primary);
        }
        attachers[entryIndex] = [plugin, primary, ...rest];
      }
    }
  }
};
var unified = new Processor().freeze();
function assertParser(name2, value) {
  if (typeof value !== "function") {
    throw new TypeError("Cannot `" + name2 + "` without `parser`");
  }
}
function assertCompiler(name2, value) {
  if (typeof value !== "function") {
    throw new TypeError("Cannot `" + name2 + "` without `compiler`");
  }
}
function assertUnfrozen(name2, frozen) {
  if (frozen) {
    throw new Error(
      "Cannot call `" + name2 + "` on a frozen processor.\nCreate a new processor first, by calling it: use `processor()` instead of `processor`."
    );
  }
}
function assertNode(node2) {
  if (!isPlainObject(node2) || typeof node2.type !== "string") {
    throw new TypeError("Expected node, got `" + node2 + "`");
  }
}
function assertDone(name2, asyncName, complete) {
  if (!complete) {
    throw new Error(
      "`" + name2 + "` finished async. Use `" + asyncName + "` instead"
    );
  }
}
function vfile(value) {
  return looksLikeAVFile(value) ? value : new VFile(value);
}
function looksLikeAVFile(value) {
  return Boolean(
    value && typeof value === "object" && "message" in value && "messages" in value
  );
}
function looksLikeAValue(value) {
  return typeof value === "string" || isUint8Array2(value);
}
function isUint8Array2(value) {
  return Boolean(
    value && typeof value === "object" && "byteLength" in value && "byteOffset" in value
  );
}

// node_modules/react-markdown/lib/index.js
var changelog = "https://github.com/remarkjs/react-markdown/blob/main/changelog.md";
var emptyPlugins = [];
var emptyRemarkRehypeOptions = { allowDangerousHtml: true };
var safeProtocol = /^(https?|ircs?|mailto|xmpp)$/i;
var deprecations = [
  { from: "astPlugins", id: "remove-buggy-html-in-markdown-parser" },
  { from: "allowDangerousHtml", id: "remove-buggy-html-in-markdown-parser" },
  {
    from: "allowNode",
    id: "replace-allownode-allowedtypes-and-disallowedtypes",
    to: "allowElement"
  },
  {
    from: "allowedTypes",
    id: "replace-allownode-allowedtypes-and-disallowedtypes",
    to: "allowedElements"
  },
  { from: "className", id: "remove-classname" },
  {
    from: "disallowedTypes",
    id: "replace-allownode-allowedtypes-and-disallowedtypes",
    to: "disallowedElements"
  },
  { from: "escapeHtml", id: "remove-buggy-html-in-markdown-parser" },
  { from: "includeElementIndex", id: "#remove-includeelementindex" },
  {
    from: "includeNodeIndex",
    id: "change-includenodeindex-to-includeelementindex"
  },
  { from: "linkTarget", id: "remove-linktarget" },
  { from: "plugins", id: "change-plugins-to-remarkplugins", to: "remarkPlugins" },
  { from: "rawSourcePos", id: "#remove-rawsourcepos" },
  { from: "renderers", id: "change-renderers-to-components", to: "components" },
  { from: "source", id: "change-source-to-children", to: "children" },
  { from: "sourcePos", id: "#remove-sourcepos" },
  { from: "transformImageUri", id: "#add-urltransform", to: "urlTransform" },
  { from: "transformLinkUri", id: "#add-urltransform", to: "urlTransform" }
];
function Markdown(options) {
  const processor = createProcessor(options);
  const file = createFile(options);
  return post(processor.runSync(processor.parse(file), file), options);
}
function createProcessor(options) {
  const rehypePlugins = options.rehypePlugins || emptyPlugins;
  const remarkPlugins = options.remarkPlugins || emptyPlugins;
  const remarkRehypeOptions = options.remarkRehypeOptions ? { ...options.remarkRehypeOptions, ...emptyRemarkRehypeOptions } : emptyRemarkRehypeOptions;
  const processor = unified().use(remarkParse).use(remarkPlugins).use(remarkRehype, remarkRehypeOptions).use(rehypePlugins);
  return processor;
}
function createFile(options) {
  const children = options.children || "";
  const file = new VFile();
  if (typeof children === "string") {
    file.value = children;
  } else {
    unreachable(
      "Unexpected value `" + children + "` for `children` prop, expected `string`"
    );
  }
  return file;
}
function post(tree, options) {
  const allowedElements = options.allowedElements;
  const allowElement = options.allowElement;
  const components = options.components;
  const disallowedElements = options.disallowedElements;
  const skipHtml = options.skipHtml;
  const unwrapDisallowed = options.unwrapDisallowed;
  const urlTransform = options.urlTransform || defaultUrlTransform;
  for (const deprecation of deprecations) {
    if (Object.hasOwn(options, deprecation.from)) {
      unreachable(
        "Unexpected `" + deprecation.from + "` prop, " + (deprecation.to ? "use `" + deprecation.to + "` instead" : "remove it") + " (see <" + changelog + "#" + deprecation.id + "> for more info)"
      );
    }
  }
  if (allowedElements && disallowedElements) {
    unreachable(
      "Unexpected combined `allowedElements` and `disallowedElements`, expected one or the other"
    );
  }
  visit(tree, transform);
  return toJsxRuntime(tree, {
    Fragment: Fragment2,
    components,
    ignoreInvalidStyle: true,
    jsx: jsx3,
    jsxs: jsxs2,
    passKeys: true,
    passNode: true
  });
  function transform(node2, index3, parent) {
    if (node2.type === "raw" && parent && typeof index3 === "number") {
      if (skipHtml) {
        parent.children.splice(index3, 1);
      } else {
        parent.children[index3] = { type: "text", value: node2.value };
      }
      return index3;
    }
    if (node2.type === "element") {
      let key;
      for (key in urlAttributes) {
        if (Object.hasOwn(urlAttributes, key) && Object.hasOwn(node2.properties, key)) {
          const value = node2.properties[key];
          const test = urlAttributes[key];
          if (test === null || test.includes(node2.tagName)) {
            node2.properties[key] = urlTransform(String(value || ""), key, node2);
          }
        }
      }
    }
    if (node2.type === "element") {
      let remove = allowedElements ? !allowedElements.includes(node2.tagName) : disallowedElements ? disallowedElements.includes(node2.tagName) : false;
      if (!remove && allowElement && typeof index3 === "number") {
        remove = !allowElement(node2, index3, parent);
      }
      if (remove && parent && typeof index3 === "number") {
        if (unwrapDisallowed && node2.children) {
          parent.children.splice(index3, 1, ...node2.children);
        } else {
          parent.children.splice(index3, 1);
        }
        return index3;
      }
    }
  }
}
function defaultUrlTransform(value) {
  const colon = value.indexOf(":");
  const questionMark = value.indexOf("?");
  const numberSign = value.indexOf("#");
  const slash = value.indexOf("/");
  if (
    // If there is no protocol, it’s relative.
    colon === -1 || // If the first colon is after a `?`, `#`, or `/`, it’s not a protocol.
    slash !== -1 && colon > slash || questionMark !== -1 && colon > questionMark || numberSign !== -1 && colon > numberSign || // It is a protocol, it should be allowed.
    safeProtocol.test(value.slice(0, colon))
  ) {
    return value;
  }
  return "";
}

// node_modules/ccount/index.js
function ccount(value, character) {
  const source = String(value);
  if (typeof character !== "string") {
    throw new TypeError("Expected character");
  }
  let count = 0;
  let index3 = source.indexOf(character);
  while (index3 !== -1) {
    count++;
    index3 = source.indexOf(character, index3 + character.length);
  }
  return count;
}

// node_modules/escape-string-regexp/index.js
function escapeStringRegexp(string3) {
  if (typeof string3 !== "string") {
    throw new TypeError("Expected a string");
  }
  return string3.replace(/[|\\{}()[\]^$+*?.]/g, "\\$&").replace(/-/g, "\\x2d");
}

// node_modules/mdast-util-find-and-replace/lib/index.js
function findAndReplace(tree, list4, options) {
  const settings = options || {};
  const ignored = convert(settings.ignore || []);
  const pairs = toPairs(list4);
  let pairIndex = -1;
  while (++pairIndex < pairs.length) {
    visitParents(tree, visitor);
  }
  function visitor(node2, parents, index3) {
    if (node2.type === "text" && typeof index3 === "number") {
      return handler(node2, parents, index3);
    }
    if (ignored(node2, index3, parents[parents.length - 1])) return SKIP;
  }
  function handler(node2, parents, index3) {
    const parent = parents[parents.length - 1];
    const find2 = pairs[pairIndex][0];
    const replace2 = pairs[pairIndex][1];
    let start2 = 0;
    let change = false;
    let nodes = [];
    find2.lastIndex = 0;
    let match = find2.exec(node2.value);
    while (match) {
      const position3 = match.index;
      const matchObject = {
        index: match.index,
        input: match.input,
        stack: [...parents, node2]
      };
      let value = replace2(...match, matchObject);
      if (typeof value === "string") {
        value = value.length > 0 ? { type: "text", value } : void 0;
      }
      if (value === false) {
        find2.lastIndex = position3 + 1;
      } else {
        if (start2 !== position3) {
          add({ type: "text", value: node2.value.slice(start2, position3) });
        }
        if (Array.isArray(value)) {
          for (const child of value) add(child);
        } else if (value) {
          add(value);
        }
        start2 = position3 + match[0].length;
        change = true;
      }
      if (!find2.global) {
        break;
      }
      match = find2.exec(node2.value);
    }
    if (change) {
      if (start2 < node2.value.length) {
        add({ type: "text", value: node2.value.slice(start2) });
      }
      parent.children.splice(index3, 1, ...nodes);
    } else {
      nodes = [node2];
    }
    return index3 + nodes.length;
    function add(child) {
      const previous3 = nodes[nodes.length - 1];
      if (previous3 && previous3.type === "text" && child.type === "text") {
        previous3.value += child.value;
      } else {
        nodes.push(child);
      }
    }
  }
}
function toPairs(tupleOrList) {
  if (!Array.isArray(tupleOrList)) {
    throw new TypeError("Expected find and replace tuple or list of tuples");
  }
  const result = [];
  const list4 = !tupleOrList[0] || Array.isArray(tupleOrList[0]) ? (
    /** @type {FindAndReplaceList} */
    tupleOrList
  ) : [
    /** @type {FindAndReplaceTuple} */
    tupleOrList
  ];
  let index3 = -1;
  while (++index3 < list4.length) {
    const tuple = list4[index3];
    result.push([toExpression(tuple[0]), toFunction(tuple[1])]);
  }
  return result;
}
function toExpression(find2) {
  return typeof find2 === "string" ? new RegExp(escapeStringRegexp(find2), "g") : find2;
}
function toFunction(replace2) {
  return typeof replace2 === "function" ? replace2 : function() {
    return replace2;
  };
}

// node_modules/mdast-util-gfm-autolink-literal/lib/index.js
var inConstruct = "phrasing";
var notInConstruct = ["autolink", "link", "image", "label"];
function gfmAutolinkLiteralFromMarkdown() {
  return {
    transforms: [transformGfmAutolinkLiterals],
    enter: {
      literalAutolink: enterLiteralAutolink,
      literalAutolinkEmail: enterLiteralAutolinkValue,
      literalAutolinkHttp: enterLiteralAutolinkValue,
      literalAutolinkWww: enterLiteralAutolinkValue
    },
    exit: {
      literalAutolink: exitLiteralAutolink,
      literalAutolinkEmail: exitLiteralAutolinkEmail,
      literalAutolinkHttp: exitLiteralAutolinkHttp,
      literalAutolinkWww: exitLiteralAutolinkWww
    }
  };
}
function gfmAutolinkLiteralToMarkdown() {
  return {
    unsafe: [
      {
        character: "@",
        before: "[+\\-.\\w]",
        after: "[\\-.\\w]",
        inConstruct,
        notInConstruct
      },
      {
        character: ".",
        before: "[Ww]",
        after: "[\\-.\\w]",
        inConstruct,
        notInConstruct
      },
      {
        character: ":",
        before: "[ps]",
        after: "\\/",
        inConstruct,
        notInConstruct
      }
    ]
  };
}
function enterLiteralAutolink(token) {
  this.enter(
    { type: "link", title: null, url: "", children: [], position: void 0 },
    token
  );
}
function enterLiteralAutolinkValue(token) {
  this.config.enter.autolinkProtocol.call(this, token);
}
function exitLiteralAutolinkHttp(token) {
  this.config.exit.autolinkProtocol.call(this, token);
}
function exitLiteralAutolinkWww(token) {
  this.config.exit.data.call(this, token);
  const node2 = this.stack[this.stack.length - 1];
  ok(node2.type === "link");
  node2.url = "http://" + this.sliceSerialize(token);
}
function exitLiteralAutolinkEmail(token) {
  this.config.exit.autolinkEmail.call(this, token);
}
function exitLiteralAutolink(token) {
  this.exit(token);
}
function transformGfmAutolinkLiterals(tree) {
  findAndReplace(
    tree,
    [
      [
        /(?<![\da-z])(https?:\/\/|www(?=\.))([\w-]*\.[\w\-.]*)([^\t\n\r ]*)/gi,
        findUrl
      ],
      [
        /(?<![\w+\-.@])xmpp:([\w+\-.]+)@([\w-]+(?:\.[\w-]+)+)(\/(?:[\w\-/]|\.(?=[\dA-Za-z]))*)?/g,
        findXmpp
      ],
      [/(?<![\w+\-.@])mailto:([\w+\-.]+)@([\w-]+(?:\.[\w-]+)+)/g, findMailto],
      [
        /(?<![\w+\-.]|(?:^|[^\d@A-Za-z])(?:mailto|xmpp):)([\w+\-.]+)@([\w-]+(?:\.[\w-]+)+)/g,
        findEmail
      ]
    ],
    { ignore: ["link", "linkReference"] }
  );
}
function findUrl(_, protocol, domain2, path2, match) {
  if (!previous2(match)) {
    return false;
  }
  let prefix = "";
  if (/^w/i.test(protocol)) {
    domain2 = protocol + domain2;
    protocol = "";
    prefix = "http://";
  }
  if (unicodePunctuation(domain2.charCodeAt(0)) || !isCorrectDomain(domain2)) {
    return false;
  }
  const parts = splitUrl(domain2 + path2);
  const result = {
    type: "link",
    title: null,
    url: prefix + protocol + parts[0],
    children: [{ type: "text", value: protocol + parts[0], position: void 0 }],
    position: void 0
  };
  if (parts[1]) {
    return [result, { type: "text", value: parts[1], position: void 0 }];
  }
  return result;
}
function findEmail(_, atext, label) {
  if (/[\d\-_]$/.test(label)) {
    return false;
  }
  return {
    type: "link",
    title: null,
    url: "mailto:" + atext + "@" + label,
    children: [{ type: "text", value: atext + "@" + label, position: void 0 }],
    position: void 0
  };
}
function findMailto(_, atext, label) {
  if (/[\d\-_]$/.test(label)) {
    return false;
  }
  const value = "mailto:" + atext + "@" + label;
  return {
    type: "link",
    title: null,
    url: value,
    children: [{ type: "text", value, position: void 0 }],
    position: void 0
  };
}
function findXmpp(matchValue, atext, label, resource, match) {
  if (/[\d\-_]$/.test(label) || resource && (!/[A-Za-z]$/.test(resource) || match.input.charAt(match.index + matchValue.length) === "@")) {
    return false;
  }
  const value = "xmpp:" + atext + "@" + label + (resource || "");
  return {
    type: "link",
    title: null,
    url: value,
    children: [{ type: "text", value, position: void 0 }],
    position: void 0
  };
}
function isCorrectDomain(domain2) {
  const parts = domain2.split(".");
  if (parts.length < 2 || parts[parts.length - 1] && (/_/.test(parts[parts.length - 1]) || !/[\dA-Za-z]/.test(parts[parts.length - 1])) || parts[parts.length - 2] && (/_/.test(parts[parts.length - 2]) || !/[\dA-Za-z]/.test(parts[parts.length - 2]))) {
    return false;
  }
  return true;
}
function splitUrl(url) {
  let index3 = url.length;
  while (index3 > 0) {
    const code4 = url.charCodeAt(index3 - 1);
    if (code4 === 33 || code4 === 34 || code4 === 38 || code4 === 39 || code4 === 41 || code4 === 44 || code4 === 46 || code4 === 58 || code4 === 59 || code4 === 60 || code4 === 62 || code4 === 63 || code4 === 93 || code4 === 125) {
      index3--;
    } else {
      break;
    }
  }
  if (index3 === url.length) {
    return [url, void 0];
  }
  let trail2 = url.slice(index3);
  url = url.slice(0, index3);
  let closingParenIndex = trail2.indexOf(")");
  const openingParens = ccount(url, "(");
  let closingParens = ccount(url, ")");
  while (closingParenIndex !== -1 && openingParens > closingParens) {
    url += trail2.slice(0, closingParenIndex + 1);
    trail2 = trail2.slice(closingParenIndex + 1);
    closingParenIndex = trail2.indexOf(")");
    closingParens++;
  }
  return [url, trail2];
}
function previous2(match) {
  const code4 = match.input.charCodeAt(match.index - 1);
  return match.index === 0 || unicodeWhitespace(code4) || unicodePunctuation(code4);
}

// node_modules/mdast-util-gfm-footnote/lib/index.js
footnoteReference2.peek = footnoteReferencePeek;
function enterFootnoteCallString() {
  this.buffer();
}
function enterFootnoteCall(token) {
  this.enter({ type: "footnoteReference", identifier: "", label: "" }, token);
}
function enterFootnoteDefinitionLabelString() {
  this.buffer();
}
function enterFootnoteDefinition(token) {
  this.enter(
    { type: "footnoteDefinition", identifier: "", label: "", children: [] },
    token
  );
}
function exitFootnoteCallString(token) {
  const label = this.resume();
  const node2 = this.stack[this.stack.length - 1];
  ok(node2.type === "footnoteReference");
  node2.identifier = normalizeIdentifier(
    this.sliceSerialize(token)
  ).toLowerCase();
  node2.label = label;
}
function exitFootnoteCall(token) {
  this.exit(token);
}
function exitFootnoteDefinitionLabelString(token) {
  const label = this.resume();
  const node2 = this.stack[this.stack.length - 1];
  ok(node2.type === "footnoteDefinition");
  node2.identifier = normalizeIdentifier(
    this.sliceSerialize(token)
  ).toLowerCase();
  node2.label = label;
}
function exitFootnoteDefinition(token) {
  this.exit(token);
}
function footnoteReferencePeek() {
  return "[";
}
function footnoteReference2(node2, _, state2, info) {
  const tracker = state2.createTracker(info);
  let value = tracker.move("[^");
  const exit3 = state2.enter("footnoteReference");
  const subexit = state2.enter("reference");
  value += tracker.move(
    state2.safe(state2.associationId(node2), { after: "]", before: value })
  );
  subexit();
  exit3();
  value += tracker.move("]");
  return value;
}
function gfmFootnoteFromMarkdown() {
  return {
    enter: {
      gfmFootnoteCallString: enterFootnoteCallString,
      gfmFootnoteCall: enterFootnoteCall,
      gfmFootnoteDefinitionLabelString: enterFootnoteDefinitionLabelString,
      gfmFootnoteDefinition: enterFootnoteDefinition
    },
    exit: {
      gfmFootnoteCallString: exitFootnoteCallString,
      gfmFootnoteCall: exitFootnoteCall,
      gfmFootnoteDefinitionLabelString: exitFootnoteDefinitionLabelString,
      gfmFootnoteDefinition: exitFootnoteDefinition
    }
  };
}
function gfmFootnoteToMarkdown(options) {
  let firstLineBlank = false;
  if (options && options.firstLineBlank) {
    firstLineBlank = true;
  }
  return {
    handlers: { footnoteDefinition, footnoteReference: footnoteReference2 },
    // This is on by default already.
    unsafe: [{ character: "[", inConstruct: ["label", "phrasing", "reference"] }]
  };
  function footnoteDefinition(node2, _, state2, info) {
    const tracker = state2.createTracker(info);
    let value = tracker.move("[^");
    const exit3 = state2.enter("footnoteDefinition");
    const subexit = state2.enter("label");
    value += tracker.move(
      state2.safe(state2.associationId(node2), { before: value, after: "]" })
    );
    subexit();
    value += tracker.move("]:");
    if (node2.children && node2.children.length > 0) {
      tracker.shift(4);
      value += tracker.move(
        (firstLineBlank ? "\n" : " ") + state2.indentLines(
          state2.containerFlow(node2, tracker.current()),
          firstLineBlank ? mapAll : mapExceptFirst
        )
      );
    }
    exit3();
    return value;
  }
}
function mapExceptFirst(line, index3, blank) {
  return index3 === 0 ? line : mapAll(line, index3, blank);
}
function mapAll(line, index3, blank) {
  return (blank ? "" : "    ") + line;
}

// node_modules/mdast-util-gfm-strikethrough/lib/index.js
var constructsWithoutStrikethrough = [
  "autolink",
  "destinationLiteral",
  "destinationRaw",
  "reference",
  "titleQuote",
  "titleApostrophe"
];
handleDelete.attention = attentionDelete;
handleDelete.peek = peekDelete;
function gfmStrikethroughFromMarkdown() {
  return {
    canContainEols: ["delete"],
    enter: { strikethrough: enterStrikethrough },
    exit: { strikethrough: exitStrikethrough }
  };
}
function gfmStrikethroughToMarkdown() {
  return {
    handlers: { delete: handleDelete },
    unsafe: [
      {
        character: "~",
        inConstruct: "phrasing",
        notInConstruct: constructsWithoutStrikethrough
      }
    ]
  };
}
function enterStrikethrough(token) {
  this.enter({ type: "delete", children: [], position: void 0 }, token);
}
function exitStrikethrough(token) {
  this.exit(token);
}
function handleDelete(node2, _, state2, info) {
  const tracker = state2.createTracker(info);
  const sequence = state2.stack.includes("strikethrough") ? "~" : "~~";
  const exit3 = state2.enter("strikethrough");
  let value = tracker.move(sequence);
  value += state2.containerPhrasing(node2, {
    ...tracker.current(),
    before: value,
    after: "~"
  });
  value += tracker.move(sequence);
  exit3();
  return value;
}
function attentionDelete() {
  return { construct: "strikethrough", markers: ["~"], sizes: [2, 1] };
}
function peekDelete() {
  return "~";
}

// node_modules/markdown-table/index.js
function defaultStringLength(value) {
  return value.length;
}
function markdownTable(table2, options) {
  const settings = options || {};
  const align = (settings.align || []).concat();
  const stringLength = settings.stringLength || defaultStringLength;
  const alignments = [];
  const cellMatrix = [];
  const sizeMatrix = [];
  const longestCellByColumn = [];
  let mostCellsPerRow = 0;
  let rowIndex = -1;
  while (++rowIndex < table2.length) {
    const row3 = [];
    const sizes2 = [];
    let columnIndex2 = -1;
    if (table2[rowIndex].length > mostCellsPerRow) {
      mostCellsPerRow = table2[rowIndex].length;
    }
    while (++columnIndex2 < table2[rowIndex].length) {
      const cell = serialize2(table2[rowIndex][columnIndex2]);
      if (settings.alignDelimiters !== false) {
        const size = stringLength(cell);
        sizes2[columnIndex2] = size;
        if (longestCellByColumn[columnIndex2] === void 0 || size > longestCellByColumn[columnIndex2]) {
          longestCellByColumn[columnIndex2] = size;
        }
      }
      row3.push(cell);
    }
    cellMatrix[rowIndex] = row3;
    sizeMatrix[rowIndex] = sizes2;
  }
  let columnIndex = -1;
  if (typeof align === "object" && "length" in align) {
    while (++columnIndex < mostCellsPerRow) {
      alignments[columnIndex] = toAlignment(align[columnIndex]);
    }
  } else {
    const code4 = toAlignment(align);
    while (++columnIndex < mostCellsPerRow) {
      alignments[columnIndex] = code4;
    }
  }
  columnIndex = -1;
  const row2 = [];
  const sizes = [];
  while (++columnIndex < mostCellsPerRow) {
    const code4 = alignments[columnIndex];
    let before = "";
    let after = "";
    if (code4 === 99) {
      before = ":";
      after = ":";
    } else if (code4 === 108) {
      before = ":";
    } else if (code4 === 114) {
      after = ":";
    }
    let size = settings.alignDelimiters === false ? 1 : Math.max(
      1,
      longestCellByColumn[columnIndex] - before.length - after.length
    );
    const cell = before + "-".repeat(size) + after;
    if (settings.alignDelimiters !== false) {
      size = before.length + size + after.length;
      if (size > longestCellByColumn[columnIndex]) {
        longestCellByColumn[columnIndex] = size;
      }
      sizes[columnIndex] = size;
    }
    row2[columnIndex] = cell;
  }
  cellMatrix.splice(1, 0, row2);
  sizeMatrix.splice(1, 0, sizes);
  rowIndex = -1;
  const lines = [];
  while (++rowIndex < cellMatrix.length) {
    const row3 = cellMatrix[rowIndex];
    const sizes2 = sizeMatrix[rowIndex];
    const cellsInRow = settings.fillRows === false ? Math.max(1, cellMatrix[rowIndex === 1 ? 0 : rowIndex].length) : mostCellsPerRow;
    columnIndex = -1;
    const line = [];
    while (++columnIndex < cellsInRow) {
      const cell = row3[columnIndex] || "";
      let before = "";
      let after = "";
      if (settings.alignDelimiters !== false) {
        const size = longestCellByColumn[columnIndex] - (sizes2[columnIndex] || 0);
        const code4 = alignments[columnIndex];
        if (code4 === 114) {
          before = " ".repeat(size);
        } else if (code4 === 99) {
          const half = size / 2;
          if (size % 2) {
            before = " ".repeat(half + 0.5);
            after = " ".repeat(half - 0.5);
          } else {
            before = " ".repeat(half);
            after = before;
          }
        } else {
          after = " ".repeat(size);
        }
      }
      if (settings.delimiterStart !== false && !columnIndex) {
        line.push("|");
      }
      if (settings.padding !== false && // Don’t add the opening space if we’re not aligning and the cell is
      // empty: there will be a closing space.
      !(settings.alignDelimiters === false && cell === "") && (settings.delimiterStart !== false || columnIndex)) {
        line.push(" ");
      }
      if (settings.alignDelimiters !== false) {
        line.push(before);
      }
      line.push(cell);
      if (settings.alignDelimiters !== false) {
        line.push(after);
      }
      if (settings.padding !== false) {
        line.push(" ");
      }
      if (settings.delimiterEnd !== false || columnIndex !== cellsInRow - 1) {
        line.push("|");
      }
    }
    lines.push(
      settings.delimiterEnd === false ? line.join("").replace(/ +$/, "") : line.join("")
    );
  }
  return lines.join("\n");
}
function serialize2(value) {
  return value === null || value === void 0 ? "" : String(value);
}
function toAlignment(value) {
  const code4 = typeof value === "string" ? value.codePointAt(0) : 0;
  return code4 === 67 || code4 === 99 ? 99 : code4 === 76 || code4 === 108 ? 108 : code4 === 82 || code4 === 114 ? 114 : 0;
}

// node_modules/mdast-util-phrasing/lib/index.js
var phrasing = (
  /** @type {(node?: unknown) => node is Exclude<PhrasingContent, Html>} */
  convert([
    "break",
    "delete",
    "emphasis",
    // To do: next major: removed since footnotes were added to GFM.
    "footnote",
    "footnoteReference",
    "image",
    "imageReference",
    "inlineCode",
    // Enabled by `mdast-util-math`:
    "inlineMath",
    "link",
    "linkReference",
    // Enabled by `mdast-util-mdx`:
    "mdxJsxTextElement",
    // Enabled by `mdast-util-mdx`:
    "mdxTextExpression",
    "strong",
    "text",
    // Enabled by `mdast-util-directive`:
    "textDirective"
  ])
);

// node_modules/mdast-util-to-markdown/lib/handle/blockquote.js
function blockquote2(node2, _, state2, info) {
  const exit3 = state2.enter("blockquote");
  const tracker = state2.createTracker(info);
  tracker.move("> ");
  tracker.shift(2);
  const value = state2.indentLines(
    state2.containerFlow(node2, tracker.current()),
    map
  );
  exit3();
  return value;
}
function map(line, _, blank) {
  return ">" + (blank ? "" : " ") + line;
}

// node_modules/mdast-util-to-markdown/lib/util/pattern-in-scope.js
function patternInScope(stack, pattern) {
  return listInScope(stack, pattern.inConstruct, true) && !listInScope(stack, pattern.notInConstruct, false);
}
function listInScope(stack, list4, none) {
  if (typeof list4 === "string") {
    list4 = [list4];
  }
  if (!list4 || list4.length === 0) {
    return none;
  }
  let index3 = -1;
  while (++index3 < list4.length) {
    if (stack.includes(list4[index3])) {
      return true;
    }
  }
  return false;
}

// node_modules/mdast-util-to-markdown/lib/handle/break.js
function hardBreak2(_, _1, state2, info) {
  let index3 = -1;
  while (++index3 < state2.unsafe.length) {
    const pattern = state2.unsafe[index3];
    if (pattern.character === "\n" && !pattern.before && !pattern.after && patternInScope(state2.stack, pattern)) {
      return /[\t ]/.test(info.before) ? "" : " ";
    }
  }
  return "\\\n";
}

// node_modules/longest-streak/index.js
function longestStreak(value, substring) {
  const source = String(value);
  let index3 = source.indexOf(substring);
  let expected = index3;
  let count = 0;
  let max = 0;
  if (typeof substring !== "string") {
    throw new TypeError("Expected substring");
  }
  while (index3 !== -1) {
    if (index3 === expected) {
      if (++count > max) {
        max = count;
      }
    } else {
      count = 1;
    }
    expected = index3 + substring.length;
    index3 = source.indexOf(substring, expected);
  }
  return max;
}

// node_modules/mdast-util-to-markdown/lib/util/format-code-as-indented.js
function formatCodeAsIndented(node2, state2) {
  return Boolean(
    state2.options.fences === false && node2.value && // If there’s no info…
    !node2.lang && // And there’s a non-whitespace character…
    /[^\n\r ]/.test(node2.value) && // And the value doesn’t start or end in a blank…
    !/^[\t ]*(?:[\n\r]|$)|(?:^|[\n\r])[\t ]*$/.test(node2.value)
  );
}

// node_modules/mdast-util-to-markdown/lib/util/check-fence.js
function checkFence(state2) {
  const marker = state2.options.fence || "`";
  if (marker !== "`" && marker !== "~") {
    throw new Error(
      "Cannot serialize code with `" + marker + "` for `options.fence`, expected `` ` `` or `~`"
    );
  }
  return marker;
}

// node_modules/mdast-util-to-markdown/lib/handle/code.js
function code2(node2, _, state2, info) {
  const marker = checkFence(state2);
  const raw = node2.value || "";
  if (formatCodeAsIndented(node2, state2)) {
    const exit4 = state2.enter("codeIndented");
    const value2 = state2.indentLines(raw, map2);
    exit4();
    return value2;
  }
  const tracker = state2.createTracker(info);
  const sequence = marker.repeat(Math.max(longestStreak(raw, marker) + 1, 3));
  const exit3 = state2.enter("codeFenced");
  const suffix = marker === "`" ? "GraveAccent" : "Tilde";
  let value = tracker.move(sequence);
  if (node2.lang) {
    const subexit = state2.enter(`codeFencedLang${suffix}`);
    value += tracker.move(
      state2.safe(node2.lang, {
        before: value,
        after: " ",
        encode: ["`"],
        ...tracker.current()
      })
    );
    subexit();
  }
  if (node2.lang && node2.meta) {
    const subexit = state2.enter(`codeFencedMeta${suffix}`);
    value += tracker.move(" ");
    value += tracker.move(
      state2.safe(node2.meta, {
        before: value,
        after: "\n",
        encode: ["`"],
        ...tracker.current()
      })
    );
    subexit();
  }
  value += tracker.move("\n");
  if (raw) {
    value += tracker.move(raw + "\n");
  }
  value += tracker.move(sequence);
  exit3();
  return value;
}
function map2(line, _, blank) {
  return (blank ? "" : "    ") + line;
}

// node_modules/mdast-util-to-markdown/lib/util/check-quote.js
function checkQuote(state2) {
  const marker = state2.options.quote || '"';
  if (marker !== '"' && marker !== "'") {
    throw new Error(
      "Cannot serialize title with `" + marker + "` for `options.quote`, expected `\"`, or `'`"
    );
  }
  return marker;
}

// node_modules/mdast-util-to-markdown/lib/handle/definition.js
function definition2(node2, _, state2, info) {
  const quote = checkQuote(state2);
  const suffix = quote === '"' ? "Quote" : "Apostrophe";
  const exit3 = state2.enter("definition");
  let subexit = state2.enter("label");
  const tracker = state2.createTracker(info);
  let value = tracker.move("[");
  value += tracker.move(
    state2.safe(state2.associationId(node2), {
      before: value,
      after: "]",
      ...tracker.current()
    })
  );
  value += tracker.move("]: ");
  subexit();
  if (
    // If there’s no url, or…
    !node2.url || // If there are control characters or whitespace.
    /[\0- \u007F]/.test(node2.url)
  ) {
    subexit = state2.enter("destinationLiteral");
    value += tracker.move("<");
    value += tracker.move(
      state2.safe(node2.url, { before: value, after: ">", ...tracker.current() })
    );
    value += tracker.move(">");
  } else {
    subexit = state2.enter("destinationRaw");
    value += tracker.move(
      state2.safe(node2.url, {
        before: value,
        after: node2.title ? " " : "\n",
        ...tracker.current()
      })
    );
  }
  subexit();
  if (node2.title) {
    subexit = state2.enter(`title${suffix}`);
    value += tracker.move(" " + quote);
    value += tracker.move(
      state2.safe(node2.title, {
        before: value,
        after: quote,
        ...tracker.current()
      })
    );
    value += tracker.move(quote);
    subexit();
  }
  exit3();
  return value;
}

// node_modules/mdast-util-to-markdown/lib/util/check-emphasis.js
function checkEmphasis(state2) {
  const marker = state2.options.emphasis || "*";
  if (marker !== "*" && marker !== "_") {
    throw new Error(
      "Cannot serialize emphasis with `" + marker + "` for `options.emphasis`, expected `*`, or `_`"
    );
  }
  return marker;
}

// node_modules/mdast-util-to-markdown/lib/util/character.js
function firstCharacter(value) {
  const code4 = value.codePointAt(0);
  return code4 === void 0 ? "" : String.fromCodePoint(code4);
}
function lastCharacter(value) {
  const code4 = value.codePointAt(value.length - 2);
  return code4 !== void 0 && code4 > 65535 ? value.slice(-2) : value.slice(-1);
}

// node_modules/mdast-util-to-markdown/lib/util/encode-character-reference.js
function encodeCharacterReference(code4) {
  return "&#x" + code4.toString(16).toUpperCase() + ";";
}

// node_modules/mdast-util-to-markdown/lib/util/encode-info.js
function encodeInfo(outside, inside, marker) {
  const outsideKind = classifyCharacter(outside);
  const insideKind = classifyCharacter(inside);
  if (outsideKind === void 0) {
    return insideKind === void 0 ? (
      // Letter inside:
      // we have to encode *both* letters for `_` as it is looser.
      // it already forms for `*` (and GFMs `~`).
      marker === "_" ? { inside: true, outside: true } : { inside: false, outside: false }
    ) : insideKind === 1 ? (
      // Whitespace inside: encode both (letter, whitespace).
      { inside: true, outside: true }
    ) : (
      // Punctuation inside: encode outer (letter)
      { inside: false, outside: true }
    );
  }
  if (outsideKind === 1) {
    return insideKind === void 0 ? (
      // Letter inside: already forms.
      { inside: false, outside: false }
    ) : insideKind === 1 ? (
      // Whitespace inside: encode both (whitespace).
      { inside: true, outside: true }
    ) : (
      // Punctuation inside: already forms.
      { inside: false, outside: false }
    );
  }
  return insideKind === void 0 ? (
    // Letter inside: already forms.
    { inside: false, outside: false }
  ) : insideKind === 1 ? (
    // Whitespace inside: encode inner (whitespace).
    { inside: true, outside: false }
  ) : (
    // Punctuation inside: already forms.
    { inside: false, outside: false }
  );
}

// node_modules/mdast-util-to-markdown/lib/util/html-kind.js
function htmlKind(value) {
  if (value.charCodeAt(0) !== 60) return;
  const next = value.charCodeAt(1);
  if (next === 33) {
    const code5 = value.charCodeAt(2);
    if (code5 === 45) return 2;
    if (code5 === 91) return 5;
    if (asciiAlpha(code5)) return 4;
    return;
  }
  if (next === 63) return 3;
  const closing = next === 47;
  const start2 = closing ? 2 : 1;
  let end = start2;
  let code4 = value.charCodeAt(end);
  if (!asciiAlpha(code4)) return;
  while (code4 === 45 || asciiAlphanumeric(code4)) {
    code4 = value.charCodeAt(++end);
  }
  const name2 = value.slice(start2, end).toLowerCase();
  if (!closing && code4 !== 47 && htmlRawNames.includes(name2)) {
    return 1;
  }
  return htmlBlockNames.includes(name2) ? 6 : 7;
}

// node_modules/mdast-util-to-markdown/lib/util/container-phrasing.js
var builtins = /* @__PURE__ */ new Map([
  ["*", false],
  ["_", true]
]);
function candidates(tokens, involved, runOf) {
  const related = /* @__PURE__ */ new Set();
  for (const token of tokens) {
    const siblings = runOf.get(token);
    if (siblings && token.attention && involved.some((d) => d.attention === token.attention)) {
      for (const sibling of siblings.tokens) {
        related.add(sibling.attention);
      }
    }
  }
  const ordered = [];
  for (const token of tokens) {
    if (token.attention && related.delete(token.attention)) {
      ordered.push(token.attention);
    }
  }
  ordered.reverse();
  return ordered;
}
function check(tokens, before, after) {
  const runs = [];
  const wrong = /* @__PURE__ */ new Map();
  const runOf = /* @__PURE__ */ new Map();
  let index3 = 0;
  while (index3 < tokens.length) {
    if (!tokens[index3].attention) {
      index3++;
      continue;
    }
    const marker = tokens[index3].value.charAt(0);
    const strict = builtins.get(marker);
    let end = index3 + 1;
    while (end < tokens.length && tokens[end].attention && tokens[end].value.charAt(0) === marker) {
      end++;
    }
    const head = index3 ? tokens[index3 - 1].value : before;
    const previous3 = classifyCharacter(head.charCodeAt(head.length - 1));
    const next = classifyCharacter(
      (end < tokens.length ? tokens[end].value : after).charCodeAt(0)
    );
    const open = !next || next === 2 && Boolean(previous3);
    const close = !previous3 || previous3 === 2 && Boolean(next);
    const run = {
      tokens: tokens.slice(index3, end),
      markers: [],
      start: 0,
      end: 0,
      // Underscores are strict, they cannot open or close inside words.
      open: strict ? open && (Boolean(previous3) || !close) : open,
      close: strict ? close && (Boolean(next) || !open) : close,
      // All builtins can split runs.
      split: strict !== void 0
    };
    for (const token of run.tokens) {
      wrong.set(token, [...run.tokens]);
      runOf.set(token, run);
      let size = token.value.length;
      while (size--) run.markers.push(token);
    }
    run.end = run.markers.length;
    runs.push(run);
    index3 = end;
  }
  pair(runs, wrong);
  index3 = -1;
  while (++index3 < tokens.length) {
    const involved = wrong.get(tokens[index3]);
    if (involved) return { attention: candidates(tokens, involved, runOf), index: index3 };
  }
}
function encode(tokens, start2) {
  const token = tokens[start2 ? 0 : tokens.length - 1];
  if (!token || token.attention) return;
  const character = start2 ? firstCharacter(token.value) : lastCharacter(token.value);
  const reference = encodeCharacterReference(
    /** @type {number} */
    character.codePointAt(0)
  );
  if (start2) {
    token.value = reference + token.value.slice(character.length);
  } else {
    let value = token.value.slice(0, token.value.length - character.length);
    let escapes = 0;
    while (value.charCodeAt(value.length - 1 - escapes) === 92) {
      escapes++;
    }
    if (escapes % 2) value += "\\";
    token.value = value + reference;
  }
}
function improve(items, chosen, mistake, before, after) {
  for (const attention4 of mistake.attention) {
    const current = chosen.get(attention4) || attention4.sequences[0];
    for (const sequence of attention4.sequences) {
      if (sequence === current) continue;
      const trial = new Map(chosen);
      trial.set(attention4, sequence);
      const tokens = render(items, trial, before, after);
      const next = check(tokens, before, after);
      if (!next || next.index > mistake.index) {
        return { chosen: trial, mistake: next, tokens };
      }
    }
  }
}
function pair(runs, wrong) {
  const openersByMarker = /* @__PURE__ */ new Map();
  for (const closer of runs) {
    const marker = closer.tokens[0].value.charAt(0);
    let openers = openersByMarker.get(marker);
    if (!openers) {
      openers = [];
      openersByMarker.set(marker, openers);
    }
    let open = closer.close ? openers.length : 0;
    while (open-- && closer.start < closer.end) {
      const opener = openers[open];
      const openerSize = opener.end - opener.start;
      const closerSize = closer.end - closer.start;
      if (closer.split ? (
        // The rule of 3, on the sizes that are left.
        (opener.close || closer.open) && closerSize % 3 && !((openerSize + closerSize) % 3)
      ) : (
        // Others pair whole runs of the same size.
        openerSize !== opener.markers.length || openerSize !== closerSize
      )) {
        continue;
      }
      const size = closer.split ? openerSize > 1 && closerSize > 1 ? 2 : 1 : closerSize;
      const opening = opener.markers.slice(opener.end - size, opener.end);
      const closing = closer.markers.slice(closer.start, closer.start + size);
      const openToken = opening[0];
      const closeToken = closing[0];
      if (
        // Whole sequences:
        // the markers of a sequence are next to each other,
        // so if the first and last marker are of one sequence of that size,
        // all markers are exactly that sequence.
        openToken === opening[size - 1] && openToken.value.length === size && closeToken === closing[size - 1] && closeToken.value.length === size && // Of the same attention, so its opening and closing sequence.
        openToken.attention === closeToken.attention
      ) {
        wrong.delete(openToken);
        wrong.delete(closeToken);
      } else {
        const involved = [.../* @__PURE__ */ new Set([...opening, ...closing])];
        for (const token of involved) {
          const list4 = wrong.get(token);
          if (list4) list4.push(...involved);
        }
      }
      opener.end -= size;
      closer.start += size;
      if (opener.start === opener.end) openers.splice(open, 1);
      open = openers.length;
    }
    if (closer.open && closer.start < closer.end) openers.push(closer);
  }
}
function attentionSequences(type, markers, sizes) {
  if (markers.length === 0) {
    throw new Error(
      "Cannot serialize `" + type + "` as attention without markers, expected one or more markers"
    );
  }
  if (sizes.length === 0) {
    throw new Error(
      "Cannot serialize `" + type + "` as attention without sizes, expected one or more sizes"
    );
  }
  for (const size of sizes) {
    if (!Number.isSafeInteger(size) || size < 1) {
      throw new Error(
        "Cannot serialize `" + type + "` as attention with `" + size + "` as size, expected positive integer"
      );
    }
  }
  const sequences = [];
  for (const marker of markers) {
    if (marker.length !== 1) {
      throw new Error(
        "Cannot serialize `" + type + "` as attention with `" + marker + "` as marker, expected a single ascii character"
      );
    }
    if (!asciiPunctuation(marker.charCodeAt(0))) {
      throw new Error(
        "Cannot serialize `" + type + "` as attention with `" + marker + "` as marker, expected ascii punctuation"
      );
    }
    for (const size of sizes) {
      sequences.push(marker.repeat(size));
    }
  }
  return sequences;
}
function phrasing2(parent, state2, info) {
  const indexStack = state2.indexStack;
  const children = parent.children || [];
  const results = [];
  let index3 = -1;
  let before = info.before;
  indexStack.push(-1);
  let tracker = state2.createTracker(info);
  while (++index3 < children.length) {
    const child = children[index3];
    let after;
    indexStack[indexStack.length - 1] = index3;
    if (index3 + 1 < children.length) {
      let handle3 = state2.handle.handlers[children[index3 + 1].type];
      if (handle3 && handle3.peek) handle3 = handle3.peek;
      after = handle3 ? firstCharacter(
        handle3(children[index3 + 1], parent, state2, {
          before: "",
          after: "",
          ...tracker.current()
        })
      ) : "";
    } else {
      after = info.after;
    }
    const previous3 = results[results.length - 1];
    if (typeof previous3 === "string" && (before === "\r" || before === "\n") && child.type === "html" && htmlKind(child.value) !== 7) {
      results[results.length - 1] = previous3.replace(/(\r?\n|\r)$/, " ");
      before = " ";
      tracker = state2.createTracker(info);
      tracker.move(serialize3(results, info.before, info.after));
    }
    const handle2 = state2.handle.handlers[child.type];
    if (handle2 && handle2.attention) {
      const item = attentionItem(
        /** @type {PhrasingParents} */
        child,
        handle2.attention,
        state2,
        tracker
      );
      if (!item) continue;
      results.push(item);
      before = lastCharacter(item.sequences[0]);
    } else {
      const value = state2.handle(child, parent, state2, {
        ...tracker.current(),
        after,
        before
      });
      if (!value) continue;
      tracker.move(value);
      results.push(value);
      before = lastCharacter(value);
    }
  }
  indexStack.pop();
  return results;
}
function attentionItem(node2, attention4, state2, tracker) {
  const { construct, markers, sizes } = attention4(node2, state2);
  const sequences = attentionSequences(node2.type, markers, sizes);
  const sequence = sequences[0];
  const exit3 = state2.enter(construct);
  const inside = state2.createTracker(tracker.current());
  inside.move(sequence);
  const children = phrasing2(node2, state2, {
    ...inside.current(),
    before: sequence,
    after: sequence
  });
  exit3();
  if (children.length === 0) return;
  tracker.move(sequence);
  tracker.move(serialize3(children, sequence, sequence));
  tracker.move(sequence);
  return { children, sequences };
}
function fix(items, chosen, before, after) {
  let tokens = render(items, chosen, before, after);
  let mistake = check(tokens, before, after);
  while (mistake) {
    const next = improve(items, chosen, mistake, before, after);
    if (!next) return;
    chosen = next.chosen;
    tokens = next.tokens;
    mistake = next.mistake;
  }
  return tokens;
}
function guess(items, chosen, before, after) {
  let previous3 = before;
  let index3 = -1;
  while (++index3 < items.length) {
    const item = items[index3];
    if (typeof item === "string") {
      previous3 = "";
      continue;
    }
    const next = index3 === items.length - 1 ? after : "";
    const sequence = item.sequences.find(function(value) {
      const marker = value.charAt(0);
      return marker !== previous3.charAt(0) && marker !== next.charAt(0) || items.length === 1 && builtins.has(marker) && value.length > before.length;
    }) || item.sequences[0];
    if (sequence !== item.sequences[0]) chosen.set(item, sequence);
    guess(item.children, chosen, sequence, sequence);
    previous3 = sequence;
  }
}
function render(items, chosen, before, after) {
  const tokens = [];
  let encodeAfter;
  let previous3 = lastCharacter(before);
  let index3 = -1;
  while (++index3 < items.length) {
    const item = items[index3];
    if (typeof item === "string") {
      const token = { value: item, attention: void 0 };
      if (encodeAfter && encodeAfter === firstCharacter(item)) {
        encode([token], true);
      }
      encodeAfter = void 0;
      tokens.push(token);
      if (token.value) previous3 = lastCharacter(token.value);
      continue;
    }
    const sequence = chosen.get(item) || item.sequences[0];
    const marker = sequence.charAt(0);
    const next = items[index3 + 1];
    const outsideBefore = previous3;
    const outsideAfter = firstCharacter(
      next === void 0 ? after : typeof next === "string" ? next : chosen.get(next) || next.sequences[0]
    );
    const inside = render(item.children, chosen, sequence, sequence);
    const head = inside[0].value;
    const open = encodeInfo(
      outsideBefore.charCodeAt(outsideBefore.length - 1),
      head.charCodeAt(0),
      marker
    );
    if (open.inside) encode(inside, true);
    const tail = lastCharacter(inside[inside.length - 1].value);
    const close = encodeInfo(
      outsideAfter.charCodeAt(0),
      tail.charCodeAt(tail.length - 1),
      marker
    );
    if (close.inside) encode(inside, false);
    if (open.outside && outsideBefore !== "\n" && outsideBefore !== "\r") {
      encode(tokens, false);
    }
    if (close.outside) encodeAfter = outsideAfter;
    tokens.push({ value: sequence, attention: item }, ...inside, {
      value: sequence,
      attention: item
    });
    previous3 = sequence.charAt(sequence.length - 1);
  }
  return tokens;
}
function serialize3(items, before, after) {
  const guessed = /* @__PURE__ */ new Map();
  guess(items, guessed, before, after);
  const tokens = (
    // Start from guesses (based on the preferred sequences).
    (guessed.size > 0 ? fix(items, guessed, before, after) : void 0) || // If stuck, start from the preferred sequences instead.
    fix(items, /* @__PURE__ */ new Map(), before, after) || // If stuck again, use the preferred sequences as they are.
    render(items, /* @__PURE__ */ new Map(), before, after)
  );
  let result = "";
  for (const token of tokens) result += token.value;
  return result;
}

// node_modules/mdast-util-to-markdown/lib/handle/emphasis.js
emphasis2.attention = attention2;
emphasis2.peek = peek;
function emphasis2(node2, _, state2, info) {
  const exit3 = state2.enter("phrasing");
  const item = attentionItem(node2, attention2, state2, state2.createTracker(info));
  const value = item ? serialize3([item], info.before, info.after) : "";
  exit3();
  return value;
}
function attention2(_, state2) {
  const marker = checkEmphasis(state2);
  return {
    construct: "emphasis",
    markers: marker === "*" ? ["*", "_"] : ["_", "*"],
    sizes: [1]
  };
}
function peek(_, _1, state2) {
  return state2.options.emphasis || "*";
}

// node_modules/mdast-util-to-markdown/lib/util/format-heading-as-setext.js
function formatHeadingAsSetext(node2, state2) {
  let literalWithBreak = false;
  visit(node2, function(node3) {
    if ("value" in node3 && /\r?\n|\r/.test(node3.value) || node3.type === "break") {
      literalWithBreak = true;
      return EXIT;
    }
  });
  return Boolean(
    (!node2.depth || node2.depth < 3) && toString(node2) && (state2.options.setext || literalWithBreak)
  );
}

// node_modules/mdast-util-to-markdown/lib/handle/heading.js
function heading2(node2, _, state2, info) {
  const rank = Math.max(Math.min(6, node2.depth || 1), 1);
  const tracker = state2.createTracker(info);
  if (formatHeadingAsSetext(node2, state2)) {
    const exit4 = state2.enter("headingSetext");
    const subexit2 = state2.enter("phrasing");
    const value2 = state2.containerPhrasing(node2, {
      ...tracker.current(),
      before: "\n",
      after: "\n"
    });
    subexit2();
    exit4();
    return value2 + "\n" + (rank === 1 ? "=" : "-").repeat(
      // The whole size…
      value2.length - // Minus the position of the character after the last EOL (or
      // 0 if there is none)…
      (Math.max(value2.lastIndexOf("\r"), value2.lastIndexOf("\n")) + 1)
    );
  }
  const sequence = "#".repeat(rank);
  const exit3 = state2.enter("headingAtx");
  const subexit = state2.enter("phrasing");
  tracker.move(sequence + " ");
  let value = state2.containerPhrasing(node2, {
    before: "# ",
    after: "\n",
    ...tracker.current()
  });
  const head = value.charCodeAt(0);
  if (head === 9 || head === 32) {
    value = encodeCharacterReference(head) + value.slice(1);
  }
  value = value ? sequence + " " + value : sequence;
  if (state2.options.closeAtx) {
    value += " " + sequence;
  }
  subexit();
  exit3();
  return value;
}

// node_modules/mdast-util-to-markdown/lib/handle/html.js
html4.peek = htmlPeek;
function html4(node2) {
  return node2.value || "";
}
function htmlPeek() {
  return "<";
}

// node_modules/mdast-util-to-markdown/lib/handle/image.js
image2.peek = imagePeek;
function image2(node2, _, state2, info) {
  const quote = checkQuote(state2);
  const suffix = quote === '"' ? "Quote" : "Apostrophe";
  const exit3 = state2.enter("image");
  let subexit = state2.enter("label");
  const tracker = state2.createTracker(info);
  let value = tracker.move("![");
  value += tracker.move(
    state2.safe(node2.alt, { before: value, after: "]", ...tracker.current() })
  );
  value += tracker.move("](");
  subexit();
  if (
    // If there’s no url but there is a title…
    !node2.url && node2.title || // If there are control characters or whitespace.
    /[\0- \u007F]/.test(node2.url)
  ) {
    subexit = state2.enter("destinationLiteral");
    value += tracker.move("<");
    value += tracker.move(
      state2.safe(node2.url, { before: value, after: ">", ...tracker.current() })
    );
    value += tracker.move(">");
  } else {
    subexit = state2.enter("destinationRaw");
    value += tracker.move(
      state2.safe(node2.url, {
        before: value,
        after: node2.title ? " " : ")",
        ...tracker.current()
      })
    );
  }
  subexit();
  if (node2.title) {
    subexit = state2.enter(`title${suffix}`);
    value += tracker.move(" " + quote);
    value += tracker.move(
      state2.safe(node2.title, {
        before: value,
        after: quote,
        ...tracker.current()
      })
    );
    value += tracker.move(quote);
    subexit();
  }
  value += tracker.move(")");
  exit3();
  return value;
}
function imagePeek() {
  return "!";
}

// node_modules/mdast-util-to-markdown/lib/handle/image-reference.js
imageReference2.peek = imageReferencePeek;
function imageReference2(node2, _, state2, info) {
  const type = node2.referenceType;
  const exit3 = state2.enter("imageReference");
  let subexit = state2.enter("label");
  const tracker = state2.createTracker(info);
  let value = tracker.move("![");
  const alt = state2.safe(node2.alt, {
    before: value,
    after: "]",
    ...tracker.current()
  });
  value += tracker.move(alt + "][");
  subexit();
  const stack = state2.stack;
  state2.stack = stack.filter(function(d) {
    return d !== "phrasing";
  });
  subexit = state2.enter("reference");
  const reference = state2.safe(state2.associationId(node2), {
    before: value,
    after: "]",
    ...tracker.current()
  });
  subexit();
  state2.stack = stack;
  exit3();
  if (type === "full" || !alt || alt !== reference) {
    value += tracker.move(reference + "]");
  } else if (type === "shortcut") {
    value = value.slice(0, -1);
  } else {
    value += tracker.move("]");
  }
  return value;
}
function imageReferencePeek() {
  return "!";
}

// node_modules/mdast-util-to-markdown/lib/handle/inline-code.js
inlineCode2.peek = inlineCodePeek;
function inlineCode2(node2, _, state2) {
  let value = node2.value || "";
  let sequence = "`";
  let index3 = -1;
  while (new RegExp("(^|[^`])" + sequence + "([^`]|$)").test(value)) {
    sequence += "`";
  }
  if (/[^\n\r ]/.test(value) && (/^[\n\r ]/.test(value) && /[\n\r ]$/.test(value) || /^`|`$/.test(value))) {
    value = " " + value + " ";
  }
  while (++index3 < state2.unsafe.length) {
    const pattern = state2.unsafe[index3];
    const expression = state2.compilePattern(pattern);
    if (!pattern.atBreak) continue;
    let match;
    while (match = expression.exec(value)) {
      let position3 = match.index;
      if (value.charCodeAt(position3) === 10 && value.charCodeAt(position3 - 1) === 13) {
        position3--;
      }
      value = value.slice(0, position3) + " " + value.slice(match.index + 1);
    }
  }
  return sequence + value + sequence;
}
function inlineCodePeek() {
  return "`";
}

// node_modules/mdast-util-to-markdown/lib/util/format-link-as-autolink.js
function formatLinkAsAutolink(node2, state2) {
  const raw = toString(node2);
  return Boolean(
    !state2.options.resourceLink && // If there’s a url…
    node2.url && // And there’s no title…
    !node2.title && // And the content of `node` is a single text node…
    node2.children && node2.children.length === 1 && node2.children[0].type === "text" && // And if the url is the same as the content…
    (raw === node2.url || "mailto:" + raw === node2.url) && // And that starts w/ a protocol…
    /^[a-z][+\-.a-z]+:/i.test(node2.url) && // And that doesn’t contain ASCII control codes (character escapes and
    // references don’t work), space, or angle brackets…
    !/[\0- <>\u007F]/.test(node2.url)
  );
}

// node_modules/mdast-util-to-markdown/lib/handle/link.js
link2.peek = linkPeek;
function link2(node2, _, state2, info) {
  const quote = checkQuote(state2);
  const suffix = quote === '"' ? "Quote" : "Apostrophe";
  const tracker = state2.createTracker(info);
  if (formatLinkAsAutolink(node2, state2)) {
    const stack = state2.stack;
    state2.stack = stack.filter(function(d) {
      return d !== "phrasing";
    });
    const exit4 = state2.enter("autolink");
    let value2 = tracker.move("<");
    value2 += tracker.move(
      state2.containerPhrasing(node2, {
        before: value2,
        after: ">",
        ...tracker.current()
      })
    );
    value2 += tracker.move(">");
    exit4();
    state2.stack = stack;
    return value2;
  }
  const exit3 = state2.enter("link");
  let subexit = state2.enter("label");
  let value = tracker.move("[");
  const text7 = state2.containerPhrasing(node2, {
    before: value,
    after: "](",
    ...tracker.current()
  });
  value += tracker.move(text7);
  subexit();
  value += tracker.move("](");
  const enclose = /[\0- \u007F]/.test(node2.url);
  const last = info.before.charAt(info.before.length - 1);
  let title = node2.title || void 0;
  if (!title && !enclose && (last === "\n" || last === "\r")) {
    let index3 = 0;
    while (index3 < text7.length && text7.charCodeAt(index3) !== 93) {
      index3 += text7.charCodeAt(index3) === 92 ? 2 : 1;
    }
    if (text7.charCodeAt(index3 + 1) === 58) title = "";
  }
  if (
    // If there’s no url but there is a title…
    !node2.url && typeof title === "string" || // If there are control characters or whitespace.
    enclose
  ) {
    subexit = state2.enter("destinationLiteral");
    value += tracker.move("<");
    value += tracker.move(
      state2.safe(node2.url, { before: value, after: ">", ...tracker.current() })
    );
    value += tracker.move(">");
  } else {
    subexit = state2.enter("destinationRaw");
    value += tracker.move(
      state2.safe(node2.url, {
        before: value,
        after: typeof title === "string" ? " " : ")",
        ...tracker.current()
      })
    );
  }
  subexit();
  if (typeof title === "string") {
    subexit = state2.enter(`title${suffix}`);
    value += tracker.move(" " + quote);
    value += tracker.move(
      state2.safe(title, {
        before: value,
        after: quote,
        ...tracker.current()
      })
    );
    value += tracker.move(quote);
    subexit();
  }
  value += tracker.move(")");
  exit3();
  return value;
}
function linkPeek(node2, _, state2) {
  return formatLinkAsAutolink(node2, state2) ? "<" : "[";
}

// node_modules/mdast-util-to-markdown/lib/handle/link-reference.js
linkReference2.peek = linkReferencePeek;
function linkReference2(node2, _, state2, info) {
  const type = node2.referenceType;
  const exit3 = state2.enter("linkReference");
  let subexit = state2.enter("label");
  const tracker = state2.createTracker(info);
  let value = tracker.move("[");
  const text7 = state2.containerPhrasing(node2, {
    before: value,
    after: "]",
    ...tracker.current()
  });
  value += tracker.move(text7 + "][");
  subexit();
  const stack = state2.stack;
  state2.stack = stack.filter(function(d) {
    return d !== "phrasing";
  });
  subexit = state2.enter("reference");
  const reference = state2.safe(state2.associationId(node2), {
    before: value,
    after: "]",
    ...tracker.current()
  });
  subexit();
  state2.stack = stack;
  exit3();
  if (type === "full" || !text7 || text7 !== reference) {
    value += tracker.move(reference + "]");
  } else if (type === "shortcut") {
    value = value.slice(0, -1);
  } else {
    value += tracker.move("]");
  }
  return value;
}
function linkReferencePeek() {
  return "[";
}

// node_modules/mdast-util-to-markdown/lib/util/check-bullet.js
function checkBullet(state2) {
  const marker = state2.options.bullet || "*";
  if (marker !== "*" && marker !== "+" && marker !== "-") {
    throw new Error(
      "Cannot serialize items with `" + marker + "` for `options.bullet`, expected `*`, `+`, or `-`"
    );
  }
  return marker;
}

// node_modules/mdast-util-to-markdown/lib/util/check-bullet-other.js
function checkBulletOther(state2) {
  const bullet = checkBullet(state2);
  const bulletOther = state2.options.bulletOther;
  if (!bulletOther) {
    return bullet === "*" ? "-" : "*";
  }
  if (bulletOther !== "*" && bulletOther !== "+" && bulletOther !== "-") {
    throw new Error(
      "Cannot serialize items with `" + bulletOther + "` for `options.bulletOther`, expected `*`, `+`, or `-`"
    );
  }
  if (bulletOther === bullet) {
    throw new Error(
      "Expected `bullet` (`" + bullet + "`) and `bulletOther` (`" + bulletOther + "`) to be different"
    );
  }
  return bulletOther;
}

// node_modules/mdast-util-to-markdown/lib/util/check-bullet-ordered.js
function checkBulletOrdered(state2) {
  const marker = state2.options.bulletOrdered || ".";
  if (marker !== "." && marker !== ")") {
    throw new Error(
      "Cannot serialize items with `" + marker + "` for `options.bulletOrdered`, expected `.` or `)`"
    );
  }
  return marker;
}

// node_modules/mdast-util-to-markdown/lib/util/check-rule.js
function checkRule(state2) {
  const marker = state2.options.rule || "*";
  if (marker !== "*" && marker !== "-" && marker !== "_") {
    throw new Error(
      "Cannot serialize rules with `" + marker + "` for `options.rule`, expected `*`, `-`, or `_`"
    );
  }
  return marker;
}

// node_modules/mdast-util-to-markdown/lib/handle/list.js
function list3(node2, parent, state2, info) {
  const exit3 = state2.enter("list");
  const bulletCurrent = state2.bulletCurrent;
  let bullet = node2.ordered ? checkBulletOrdered(state2) : checkBullet(state2);
  const bulletOther = node2.ordered ? bullet === "." ? ")" : "." : checkBulletOther(state2);
  let useDifferentMarker = parent && state2.bulletLastUsed ? bullet === state2.bulletLastUsed : false;
  if (!node2.ordered) {
    const firstListItem = node2.children ? node2.children[0] : void 0;
    if (
      // Bullet could be used as a thematic break marker:
      (bullet === "*" || bullet === "-") && // Empty first list item:
      firstListItem && (!firstListItem.children || !firstListItem.children[0]) && // Directly in two other list items:
      state2.stack[state2.stack.length - 1] === "list" && state2.stack[state2.stack.length - 2] === "listItem" && state2.stack[state2.stack.length - 3] === "list" && state2.stack[state2.stack.length - 4] === "listItem" && // That are each the first child.
      state2.indexStack[state2.indexStack.length - 1] === 0 && state2.indexStack[state2.indexStack.length - 2] === 0 && state2.indexStack[state2.indexStack.length - 3] === 0
    ) {
      useDifferentMarker = true;
    }
    if (checkRule(state2) === bullet && firstListItem) {
      let index3 = -1;
      while (++index3 < node2.children.length) {
        const item = node2.children[index3];
        if (item && item.type === "listItem" && item.children && item.children[0] && item.children[0].type === "thematicBreak") {
          useDifferentMarker = true;
          break;
        }
      }
    }
  }
  if (useDifferentMarker) {
    bullet = bulletOther;
  }
  state2.bulletCurrent = bullet;
  const value = state2.containerFlow(node2, info);
  state2.bulletLastUsed = bullet;
  state2.bulletCurrent = bulletCurrent;
  exit3();
  return value;
}

// node_modules/mdast-util-to-markdown/lib/util/check-list-item-indent.js
function checkListItemIndent(state2) {
  const style = state2.options.listItemIndent || "one";
  if (style !== "tab" && style !== "one" && style !== "mixed") {
    throw new Error(
      "Cannot serialize items with `" + style + "` for `options.listItemIndent`, expected `tab`, `one`, or `mixed`"
    );
  }
  return style;
}

// node_modules/mdast-util-to-markdown/lib/handle/list-item.js
function listItem2(node2, parent, state2, info) {
  const listItemIndent = checkListItemIndent(state2);
  let bullet = state2.bulletCurrent || checkBullet(state2);
  if (parent && parent.type === "list" && parent.ordered) {
    bullet = (typeof parent.start === "number" && parent.start > -1 ? parent.start : 1) + (state2.options.incrementListMarker === false ? 0 : parent.children.indexOf(node2)) + bullet;
  }
  let size = bullet.length + 1;
  if (listItemIndent === "tab" || listItemIndent === "mixed" && (parent && parent.type === "list" && parent.spread || node2.spread)) {
    size = Math.ceil(size / 4) * 4;
  }
  const tracker = state2.createTracker(info);
  tracker.move(bullet + " ".repeat(size - bullet.length));
  tracker.shift(size);
  const exit3 = state2.enter("listItem");
  const value = state2.indentLines(
    state2.containerFlow(node2, tracker.current()),
    map3
  );
  exit3();
  return value;
  function map3(line, index3, blank) {
    if (index3) {
      return (blank ? "" : " ".repeat(size)) + line;
    }
    return (blank ? bullet : bullet + " ".repeat(size - bullet.length)) + line;
  }
}

// node_modules/mdast-util-to-markdown/lib/handle/paragraph.js
function paragraph2(node2, _, state2, info) {
  const exit3 = state2.enter("paragraph");
  const subexit = state2.enter("phrasing");
  const value = state2.containerPhrasing(node2, info);
  subexit();
  exit3();
  return value;
}

// node_modules/mdast-util-to-markdown/lib/handle/root.js
function root3(node2, _, state2, info) {
  const hasPhrasing = node2.children.some(function(d) {
    return phrasing(d);
  });
  if (!hasPhrasing) return state2.containerFlow(node2, info);
  const exit3 = state2.enter("phrasing");
  const value = state2.containerPhrasing(node2, info);
  exit3();
  return value;
}

// node_modules/mdast-util-to-markdown/lib/util/check-strong.js
function checkStrong(state2) {
  const marker = state2.options.strong || "*";
  if (marker !== "*" && marker !== "_") {
    throw new Error(
      "Cannot serialize strong with `" + marker + "` for `options.strong`, expected `*`, or `_`"
    );
  }
  return marker;
}

// node_modules/mdast-util-to-markdown/lib/handle/strong.js
strong2.attention = attention3;
strong2.peek = peek2;
function strong2(node2, _, state2, info) {
  const exit3 = state2.enter("phrasing");
  const item = attentionItem(node2, attention3, state2, state2.createTracker(info));
  const value = item ? serialize3([item], info.before, info.after) : "";
  exit3();
  return value;
}
function attention3(_, state2) {
  const marker = checkStrong(state2);
  return {
    construct: "strong",
    markers: marker === "*" ? ["*", "_"] : ["_", "*"],
    sizes: [2]
  };
}
function peek2(_, _1, state2) {
  return state2.options.strong || "*";
}

// node_modules/mdast-util-to-markdown/lib/handle/text.js
function text5(node2, parent, state2, info) {
  const siblings = parent ? parent.children : [];
  const index3 = state2.indexStack[state2.indexStack.length - 1];
  return state2.safe(node2.value, {
    ...info,
    afterNode: siblings[index3 + 1],
    beforeNode: siblings[index3 - 1]
  });
}

// node_modules/mdast-util-to-markdown/lib/util/check-rule-repetition.js
function checkRuleRepetition(state2) {
  const repetition = state2.options.ruleRepetition || 3;
  if (repetition < 3) {
    throw new Error(
      "Cannot serialize rules with repetition `" + repetition + "` for `options.ruleRepetition`, expected `3` or more"
    );
  }
  return repetition;
}

// node_modules/mdast-util-to-markdown/lib/handle/thematic-break.js
function thematicBreak3(_, _1, state2) {
  const value = (checkRule(state2) + (state2.options.ruleSpaces ? " " : "")).repeat(checkRuleRepetition(state2));
  return state2.options.ruleSpaces ? value.slice(0, -1) : value;
}

// node_modules/mdast-util-to-markdown/lib/handle/index.js
var handle = {
  blockquote: blockquote2,
  break: hardBreak2,
  code: code2,
  definition: definition2,
  emphasis: emphasis2,
  hardBreak: hardBreak2,
  heading: heading2,
  html: html4,
  image: image2,
  imageReference: imageReference2,
  inlineCode: inlineCode2,
  link: link2,
  linkReference: linkReference2,
  list: list3,
  listItem: listItem2,
  paragraph: paragraph2,
  root: root3,
  strong: strong2,
  text: text5,
  thematicBreak: thematicBreak3
};

// node_modules/mdast-util-gfm-table/lib/index.js
function gfmTableFromMarkdown() {
  return {
    enter: {
      table: enterTable,
      tableData: enterCell,
      tableHeader: enterCell,
      tableRow: enterRow
    },
    exit: {
      codeText: exitCodeText,
      table: exitTable,
      tableData: exit2,
      tableHeader: exit2,
      tableRow: exit2
    }
  };
}
function enterTable(token) {
  const align = token._align;
  ok(align, "expected `_align` on table");
  this.enter(
    {
      type: "table",
      align: align.map(function(d) {
        return d === "none" ? null : d;
      }),
      children: [],
      position: void 0
    },
    token
  );
  this.data.inTable = true;
}
function exitTable(token) {
  this.exit(token);
  this.data.inTable = void 0;
}
function enterRow(token) {
  this.enter({ type: "tableRow", children: [], position: void 0 }, token);
}
function exit2(token) {
  this.exit(token);
}
function enterCell(token) {
  this.enter({ type: "tableCell", children: [], position: void 0 }, token);
}
function exitCodeText(token) {
  let value = this.resume();
  if (this.data.inTable) {
    value = value.replace(/\\([\\|])/g, replace);
  }
  const node2 = this.stack[this.stack.length - 1];
  ok(node2.type === "inlineCode");
  node2.value = value;
  this.exit(token);
}
function replace($0, $1) {
  return $1 === "|" ? $1 : $0;
}
function gfmTableToMarkdown(options) {
  const settings = options || {};
  const {
    tableCellPadding: padding,
    tablePipeAlign: alignDelimiters,
    stringLength
  } = settings;
  const around = padding ? " " : "|";
  return {
    handlers: {
      inlineCode: inlineCodeWithTable,
      table: handleTable,
      tableCell: handleTableCell,
      tableRow: handleTableRow
    },
    join: [joinTable],
    unsafe: [
      { character: "\r", inConstruct: "tableCell" },
      { character: "\n", inConstruct: "tableCell" },
      // A pipe, when followed by a tab or space (padding), or a dash or colon
      // (unpadded delimiter row), could result in a table.
      { atBreak: true, character: "|", after: "[	 :-]" },
      // A pipe in a cell must be encoded.
      { character: "|", inConstruct: "tableCell" },
      // A colon must be followed by a dash, in which case it could start a
      // delimiter row.
      { atBreak: true, character: ":", after: "-" },
      // A delimiter row can also start with a dash, when followed by more
      // dashes, a colon, or a pipe.
      // This is a stricter version than the built in check for lists, thematic
      // breaks, and setex heading underlines though:
      // <https://github.com/syntax-tree/mdast-util-to-markdown/blob/51a2038/lib/unsafe.js#L57>
      { atBreak: true, character: "-", after: "[:|-]" }
    ]
  };
  function handleTable(node2, _, state2, info) {
    const matrix = handleTableAsData(node2, state2, info);
    const head = matrix[0];
    if (head) {
      let columns = node2.align ? node2.align.length : 0;
      let index3 = -1;
      if (!columns) {
        while (++index3 < matrix.length) {
          if (matrix[index3].length > columns) columns = matrix[index3].length;
        }
      }
      while (head.length < columns) head.push("");
    }
    return serializeData(matrix, node2.align);
  }
  function handleTableRow(node2, _, state2, info) {
    const row2 = handleTableRowAsData(node2, state2, info);
    const value = serializeData([row2]);
    return value.slice(0, value.indexOf("\n"));
  }
  function handleTableCell(node2, _, state2, info) {
    const exit3 = state2.enter("tableCell");
    const subexit = state2.enter("phrasing");
    const value = state2.containerPhrasing(node2, {
      ...info,
      before: around,
      after: around
    });
    subexit();
    exit3();
    return value;
  }
  function serializeData(matrix, align) {
    return markdownTable(matrix, {
      align,
      alignDelimiters,
      fillRows: false,
      padding,
      stringLength
    });
  }
  function handleTableAsData(node2, state2, info) {
    const { children } = node2;
    let index3 = -1;
    const result = [];
    const subexit = state2.enter("table");
    while (++index3 < children.length) {
      result[index3] = handleTableRowAsData(children[index3], state2, info);
    }
    subexit();
    return result;
  }
  function handleTableRowAsData(node2, state2, info) {
    const { children } = node2;
    let index3 = -1;
    const result = [];
    const subexit = state2.enter("tableRow");
    while (++index3 < children.length) {
      result[index3] = handleTableCell(children[index3], node2, state2, info);
    }
    subexit();
    return result;
  }
  function inlineCodeWithTable(node2, parent, state2) {
    const value = handle.inlineCode(node2, parent, state2);
    if (!state2.stack.includes("tableCell")) return value;
    const result = [];
    let start2 = 0;
    let index3 = value.indexOf("|");
    while (index3 !== -1) {
      let before = index3;
      while (before > start2 && value.charCodeAt(before - 1) === 92) {
        before--;
      }
      result.push(value.slice(start2, index3));
      if ((index3 - before) % 2 === 0) result.push("\\");
      start2 = index3;
      index3 = value.indexOf("|", index3 + 1);
    }
    result.push(value.slice(start2));
    return result.join("");
  }
}
function joinTable(left, right, parent, state2) {
  if (left.type === "table" && parent.type === "listItem" && parent.spread === false && (right.type === "definition" || right.type === "heading" && formatHeadingAsSetext2(right, state2) || right.type === "paragraph" || right.type === "table")) {
    return 1;
  }
}
function formatHeadingAsSetext2(node2, state2) {
  let literalWithBreak = false;
  visit(node2, function(node3) {
    if (node3.type === "break" || "value" in node3 && /\r?\n|\r/.test(node3.value)) {
      literalWithBreak = true;
      return EXIT;
    }
  });
  return Boolean(
    (!node2.depth || node2.depth < 3) && toString(node2) && (state2.options.setext || literalWithBreak)
  );
}

// node_modules/mdast-util-gfm-task-list-item/lib/index.js
function gfmTaskListItemFromMarkdown() {
  return {
    exit: {
      taskListCheckValueChecked: exitCheck,
      taskListCheckValueUnchecked: exitCheck,
      paragraph: exitParagraphWithTaskListItem
    }
  };
}
function gfmTaskListItemToMarkdown() {
  return {
    handlers: { paragraph: paragraphWithTaskListItem },
    join: [joinDefinitionsBeforeTaskListItemParagraph]
  };
}
function exitCheck(token) {
  const node2 = this.stack[this.stack.length - 2];
  ok(node2.type === "listItem");
  node2.checked = token.type === "taskListCheckValueChecked";
}
function exitParagraphWithTaskListItem(token) {
  const parent = this.stack[this.stack.length - 2];
  if (parent && parent.type === "listItem" && typeof parent.checked === "boolean") {
    const node2 = this.stack[this.stack.length - 1];
    ok(node2.type === "paragraph");
    const head = node2.children[0];
    if (head && head.type === "text") {
      const siblings = parent.children;
      let index3 = -1;
      let firstParagraph;
      while (++index3 < siblings.length) {
        const sibling = siblings[index3];
        if (sibling.type === "paragraph") {
          firstParagraph = sibling;
          break;
        }
      }
      if (firstParagraph === node2) {
        head.value = head.value.slice(1);
        if (head.value.length === 0) {
          node2.children.shift();
        } else if (node2.position && head.position && typeof head.position.start.offset === "number") {
          head.position.start.column++;
          head.position.start.offset++;
          node2.position.start = { ...head.position.start };
        }
      }
    }
  }
  this.exit(token);
}
function paragraphWithTaskListItem(node2, parent, state2, info) {
  if (!parent || parent.type !== "listItem" || taskListItemParagraph(parent) !== node2) {
    return handle.paragraph(node2, parent, state2, info);
  }
  const checkbox = "[" + (parent.checked ? "x" : " ") + "] ";
  const tracker = state2.createTracker(info);
  tracker.move(checkbox);
  const value = handle.paragraph(
    node2.children.length > 0 ? node2 : { type: "paragraph", children: [{ type: "text", value: " " }] },
    parent,
    state2,
    { ...info, ...tracker.current() }
  );
  return checkbox + value;
}
function joinDefinitionsBeforeTaskListItemParagraph(left, _, parent) {
  if (!(left.type === "definition" && parent.type === "listItem")) {
    return;
  }
  const paragraph3 = taskListItemParagraph(parent);
  if (paragraph3 && parent.children.indexOf(left) < parent.children.indexOf(paragraph3)) {
    return 0;
  }
}
function taskListItemParagraph(node2) {
  if (typeof node2.checked !== "boolean") {
    return;
  }
  for (const child of node2.children) {
    if (child.type === "paragraph") {
      return child;
    }
    if (child.type !== "definition") {
      return;
    }
  }
}

// node_modules/mdast-util-gfm/lib/index.js
function gfmFromMarkdown() {
  return [
    gfmAutolinkLiteralFromMarkdown(),
    gfmFootnoteFromMarkdown(),
    gfmStrikethroughFromMarkdown(),
    gfmTableFromMarkdown(),
    gfmTaskListItemFromMarkdown()
  ];
}
function gfmToMarkdown(options) {
  return {
    extensions: [
      gfmAutolinkLiteralToMarkdown(),
      gfmFootnoteToMarkdown(options),
      gfmStrikethroughToMarkdown(),
      gfmTableToMarkdown(options),
      gfmTaskListItemToMarkdown()
    ]
  };
}

// node_modules/micromark-extension-gfm-autolink-literal/lib/syntax.js
var wwwPrefix = {
  tokenize: tokenizeWwwPrefix,
  partial: true
};
var domain = {
  tokenize: tokenizeDomain,
  partial: true
};
var path = {
  tokenize: tokenizePath,
  partial: true
};
var trail = {
  tokenize: tokenizeTrail,
  partial: true
};
var emailDomainDotTrail = {
  tokenize: tokenizeEmailDomainDotTrail,
  partial: true
};
var wwwAutolink = {
  name: "wwwAutolink",
  tokenize: tokenizeWwwAutolink,
  previous: previousWww
};
var protocolAutolink = {
  name: "protocolAutolink",
  tokenize: tokenizeProtocolAutolink,
  previous: previousProtocol
};
var emailAutolink = {
  name: "emailAutolink",
  tokenize: tokenizeEmailAutolink,
  previous: previousEmail
};
var text6 = {};
function gfmAutolinkLiteral() {
  return {
    text: text6
  };
}
var code3 = 48;
while (code3 < 123) {
  text6[code3] = emailAutolink;
  code3++;
  if (code3 === 58) code3 = 65;
  else if (code3 === 91) code3 = 97;
}
text6[43] = emailAutolink;
text6[45] = emailAutolink;
text6[46] = emailAutolink;
text6[95] = emailAutolink;
text6[72] = [emailAutolink, protocolAutolink];
text6[104] = [emailAutolink, protocolAutolink];
text6[87] = [emailAutolink, wwwAutolink];
text6[119] = [emailAutolink, wwwAutolink];
function tokenizeEmailAutolink(effects, ok3, nok) {
  const self2 = this;
  let dot;
  let data;
  return start2;
  function start2(code4) {
    if (!gfmAtext(code4) || !previousEmail.call(self2, self2.previous) || previousUnbalanced(self2.events)) {
      return nok(code4);
    }
    effects.enter("literalAutolink");
    effects.enter("literalAutolinkEmail");
    return atext(code4);
  }
  function atext(code4) {
    if (gfmAtext(code4)) {
      effects.consume(code4);
      return atext;
    }
    if (code4 === 64) {
      effects.consume(code4);
      return emailDomain;
    }
    return nok(code4);
  }
  function emailDomain(code4) {
    if (code4 === 46) {
      return effects.check(emailDomainDotTrail, emailDomainAfter, emailDomainDot)(code4);
    }
    if (code4 === 45 || code4 === 95 || asciiAlphanumeric(code4)) {
      data = true;
      effects.consume(code4);
      return emailDomain;
    }
    return emailDomainAfter(code4);
  }
  function emailDomainDot(code4) {
    effects.consume(code4);
    dot = true;
    return emailDomain;
  }
  function emailDomainAfter(code4) {
    if (data && dot && asciiAlpha(self2.previous)) {
      effects.exit("literalAutolinkEmail");
      effects.exit("literalAutolink");
      return ok3(code4);
    }
    return nok(code4);
  }
}
function tokenizeWwwAutolink(effects, ok3, nok) {
  const self2 = this;
  return wwwStart;
  function wwwStart(code4) {
    if (code4 !== 87 && code4 !== 119 || !previousWww.call(self2, self2.previous) || previousUnbalanced(self2.events)) {
      return nok(code4);
    }
    effects.enter("literalAutolink");
    effects.enter("literalAutolinkWww");
    return effects.check(wwwPrefix, effects.attempt(domain, effects.attempt(path, wwwAfter), nok), nok)(code4);
  }
  function wwwAfter(code4) {
    effects.exit("literalAutolinkWww");
    effects.exit("literalAutolink");
    return ok3(code4);
  }
}
function tokenizeProtocolAutolink(effects, ok3, nok) {
  const self2 = this;
  let buffer = "";
  let seen = false;
  return protocolStart;
  function protocolStart(code4) {
    if ((code4 === 72 || code4 === 104) && previousProtocol.call(self2, self2.previous) && !previousUnbalanced(self2.events)) {
      effects.enter("literalAutolink");
      effects.enter("literalAutolinkHttp");
      buffer += String.fromCodePoint(code4);
      effects.consume(code4);
      return protocolPrefixInside;
    }
    return nok(code4);
  }
  function protocolPrefixInside(code4) {
    if (asciiAlpha(code4) && buffer.length < 5) {
      buffer += String.fromCodePoint(code4);
      effects.consume(code4);
      return protocolPrefixInside;
    }
    if (code4 === 58) {
      const protocol = buffer.toLowerCase();
      if (protocol === "http" || protocol === "https") {
        effects.consume(code4);
        return protocolSlashesInside;
      }
    }
    return nok(code4);
  }
  function protocolSlashesInside(code4) {
    if (code4 === 47) {
      effects.consume(code4);
      if (seen) {
        return afterProtocol;
      }
      seen = true;
      return protocolSlashesInside;
    }
    return nok(code4);
  }
  function afterProtocol(code4) {
    return code4 === null || asciiControl(code4) || markdownLineEndingOrSpace(code4) || unicodeWhitespace(code4) || unicodePunctuation(code4) ? nok(code4) : effects.attempt(domain, effects.attempt(path, protocolAfter), nok)(code4);
  }
  function protocolAfter(code4) {
    effects.exit("literalAutolinkHttp");
    effects.exit("literalAutolink");
    return ok3(code4);
  }
}
function tokenizeWwwPrefix(effects, ok3, nok) {
  let size = 0;
  return wwwPrefixInside;
  function wwwPrefixInside(code4) {
    if ((code4 === 87 || code4 === 119) && size < 3) {
      size++;
      effects.consume(code4);
      return wwwPrefixInside;
    }
    if (code4 === 46 && size === 3) {
      effects.consume(code4);
      return wwwPrefixAfter;
    }
    return nok(code4);
  }
  function wwwPrefixAfter(code4) {
    return code4 === null ? nok(code4) : ok3(code4);
  }
}
function tokenizeDomain(effects, ok3, nok) {
  let underscoreInLastSegment;
  let underscoreInLastLastSegment;
  let seen;
  return domainInside;
  function domainInside(code4) {
    if (code4 === 46 || code4 === 95) {
      return effects.check(trail, domainAfter, domainAtPunctuation)(code4);
    }
    if (code4 === null || markdownLineEndingOrSpace(code4) || unicodeWhitespace(code4) || code4 !== 45 && unicodePunctuation(code4)) {
      return domainAfter(code4);
    }
    seen = true;
    effects.consume(code4);
    return domainInside;
  }
  function domainAtPunctuation(code4) {
    if (code4 === 95) {
      underscoreInLastSegment = true;
    } else {
      underscoreInLastLastSegment = underscoreInLastSegment;
      underscoreInLastSegment = void 0;
    }
    effects.consume(code4);
    return domainInside;
  }
  function domainAfter(code4) {
    if (underscoreInLastLastSegment || underscoreInLastSegment || !seen) {
      return nok(code4);
    }
    return ok3(code4);
  }
}
function tokenizePath(effects, ok3) {
  let sizeOpen = 0;
  let sizeClose = 0;
  return pathInside;
  function pathInside(code4) {
    if (code4 === 40) {
      sizeOpen++;
      effects.consume(code4);
      return pathInside;
    }
    if (code4 === 41 && sizeClose < sizeOpen) {
      return pathAtPunctuation(code4);
    }
    if (code4 === 33 || code4 === 34 || code4 === 38 || code4 === 39 || code4 === 41 || code4 === 42 || code4 === 44 || code4 === 46 || code4 === 58 || code4 === 59 || code4 === 60 || code4 === 63 || code4 === 93 || code4 === 95 || code4 === 126) {
      return effects.check(trail, ok3, pathAtPunctuation)(code4);
    }
    if (code4 === null || markdownLineEndingOrSpace(code4) || unicodeWhitespace(code4)) {
      return ok3(code4);
    }
    effects.consume(code4);
    return pathInside;
  }
  function pathAtPunctuation(code4) {
    if (code4 === 41) {
      sizeClose++;
    }
    effects.consume(code4);
    return pathInside;
  }
}
function tokenizeTrail(effects, ok3, nok) {
  return trail2;
  function trail2(code4) {
    if (code4 === 33 || code4 === 34 || code4 === 39 || code4 === 41 || code4 === 42 || code4 === 44 || code4 === 46 || code4 === 58 || code4 === 59 || code4 === 63 || code4 === 95 || code4 === 126) {
      effects.consume(code4);
      return trail2;
    }
    if (code4 === 38) {
      effects.consume(code4);
      return trailCharacterReferenceStart;
    }
    if (code4 === 93) {
      effects.consume(code4);
      return trailBracketAfter;
    }
    if (
      // `<` is an end.
      code4 === 60 || // So is whitespace.
      code4 === null || markdownLineEndingOrSpace(code4) || unicodeWhitespace(code4)
    ) {
      return ok3(code4);
    }
    return nok(code4);
  }
  function trailBracketAfter(code4) {
    if (code4 === null || code4 === 40 || code4 === 91 || markdownLineEndingOrSpace(code4) || unicodeWhitespace(code4)) {
      return ok3(code4);
    }
    return trail2(code4);
  }
  function trailCharacterReferenceStart(code4) {
    return asciiAlpha(code4) ? trailCharacterReferenceInside(code4) : nok(code4);
  }
  function trailCharacterReferenceInside(code4) {
    if (code4 === 59) {
      effects.consume(code4);
      return trail2;
    }
    if (asciiAlpha(code4)) {
      effects.consume(code4);
      return trailCharacterReferenceInside;
    }
    return nok(code4);
  }
}
function tokenizeEmailDomainDotTrail(effects, ok3, nok) {
  return start2;
  function start2(code4) {
    effects.consume(code4);
    return after;
  }
  function after(code4) {
    return asciiAlphanumeric(code4) ? nok(code4) : ok3(code4);
  }
}
function previousWww(code4) {
  return code4 === null || code4 === 40 || code4 === 42 || code4 === 95 || code4 === 91 || code4 === 93 || code4 === 126 || markdownLineEndingOrSpace(code4);
}
function previousProtocol(code4) {
  return !asciiAlpha(code4);
}
function previousEmail(code4) {
  return !(code4 === 47 || gfmAtext(code4));
}
function gfmAtext(code4) {
  return code4 === 43 || code4 === 45 || code4 === 46 || code4 === 95 || asciiAlphanumeric(code4);
}
function previousUnbalanced(events) {
  let index3 = events.length;
  let result = false;
  while (index3--) {
    const token = events[index3][1];
    if ((token.type === "labelLink" || token.type === "labelImage") && !token._balanced) {
      result = true;
      break;
    }
    if (token._gfmAutolinkLiteralWalkedInto) {
      result = false;
      break;
    }
  }
  if (events.length > 0 && !result) {
    events[events.length - 1][1]._gfmAutolinkLiteralWalkedInto = true;
  }
  return result;
}

// node_modules/micromark-extension-gfm-footnote/lib/syntax.js
var indent = {
  tokenize: tokenizeIndent2,
  partial: true
};
function gfmFootnote() {
  return {
    document: {
      [91]: {
        name: "gfmFootnoteDefinition",
        tokenize: tokenizeDefinitionStart,
        continuation: {
          tokenize: tokenizeDefinitionContinuation
        },
        exit: gfmFootnoteDefinitionEnd
      }
    },
    text: {
      [91]: {
        name: "gfmFootnoteCall",
        tokenize: tokenizeGfmFootnoteCall
      },
      [93]: {
        name: "gfmPotentialFootnoteCall",
        add: "after",
        tokenize: tokenizePotentialGfmFootnoteCall,
        resolveTo: resolveToPotentialGfmFootnoteCall
      }
    }
  };
}
function tokenizePotentialGfmFootnoteCall(effects, ok3, nok) {
  const self2 = this;
  let index3 = self2.events.length;
  const defined = self2.parser.gfmFootnotes || (self2.parser.gfmFootnotes = []);
  let labelStart;
  while (index3--) {
    const token = self2.events[index3][1];
    if (token.type === "labelImage") {
      labelStart = token;
      break;
    }
    if (token.type === "gfmFootnoteCall" || token.type === "labelLink" || token.type === "label" || token.type === "image" || token.type === "link") {
      break;
    }
  }
  return start2;
  function start2(code4) {
    if (!labelStart || !labelStart._balanced) {
      return nok(code4);
    }
    const id = normalizeIdentifier(self2.sliceSerialize({
      start: labelStart.end,
      end: self2.now()
    }));
    if (id.codePointAt(0) !== 94 || !defined.includes(id.slice(1))) {
      return nok(code4);
    }
    effects.enter("gfmFootnoteCallLabelMarker");
    effects.consume(code4);
    effects.exit("gfmFootnoteCallLabelMarker");
    return ok3(code4);
  }
}
function resolveToPotentialGfmFootnoteCall(events, context) {
  let index3 = events.length;
  let labelStart;
  while (index3--) {
    if (events[index3][1].type === "labelImage" && events[index3][0] === "enter") {
      labelStart = events[index3][1];
      break;
    }
  }
  events[index3 + 1][1].type = "data";
  events[index3 + 3][1].type = "gfmFootnoteCallLabelMarker";
  const call = {
    type: "gfmFootnoteCall",
    start: Object.assign({}, events[index3 + 3][1].start),
    end: Object.assign({}, events[events.length - 1][1].end)
  };
  const marker = {
    type: "gfmFootnoteCallMarker",
    start: Object.assign({}, events[index3 + 3][1].end),
    end: Object.assign({}, events[index3 + 3][1].end)
  };
  marker.end.column++;
  marker.end.offset++;
  marker.end._bufferIndex++;
  const string3 = {
    type: "gfmFootnoteCallString",
    start: Object.assign({}, marker.end),
    end: Object.assign({}, events[events.length - 1][1].start)
  };
  const chunk = {
    type: "chunkString",
    contentType: "string",
    start: Object.assign({}, string3.start),
    end: Object.assign({}, string3.end)
  };
  const replacement = [
    // Take the `labelImageMarker` (now `data`, the `!`)
    events[index3 + 1],
    events[index3 + 2],
    ["enter", call, context],
    // The `[`
    events[index3 + 3],
    events[index3 + 4],
    // The `^`.
    ["enter", marker, context],
    ["exit", marker, context],
    // Everything in between.
    ["enter", string3, context],
    ["enter", chunk, context],
    ["exit", chunk, context],
    ["exit", string3, context],
    // The ending (`]`, properly parsed and labelled).
    events[events.length - 2],
    events[events.length - 1],
    ["exit", call, context]
  ];
  events.splice(index3, events.length - index3 + 1, ...replacement);
  return events;
}
function tokenizeGfmFootnoteCall(effects, ok3, nok) {
  const self2 = this;
  const defined = self2.parser.gfmFootnotes || (self2.parser.gfmFootnotes = []);
  let size = 0;
  let data;
  return start2;
  function start2(code4) {
    effects.enter("gfmFootnoteCall");
    effects.enter("gfmFootnoteCallLabelMarker");
    effects.consume(code4);
    effects.exit("gfmFootnoteCallLabelMarker");
    return callStart;
  }
  function callStart(code4) {
    if (code4 !== 94) return nok(code4);
    effects.enter("gfmFootnoteCallMarker");
    effects.consume(code4);
    effects.exit("gfmFootnoteCallMarker");
    effects.enter("gfmFootnoteCallString");
    effects.enter("chunkString").contentType = "string";
    return callData;
  }
  function callData(code4) {
    if (
      // Too long.
      size > 999 || // Closing brace with nothing.
      code4 === 93 && !data || // Space or tab is not supported by GFM for some reason.
      // `\n` and `[` not being supported makes sense.
      code4 === null || code4 === 91 || markdownLineEndingOrSpace(code4)
    ) {
      return nok(code4);
    }
    if (code4 === 93) {
      effects.exit("chunkString");
      const token = effects.exit("gfmFootnoteCallString");
      if (!defined.includes(normalizeIdentifier(self2.sliceSerialize(token)))) {
        return nok(code4);
      }
      effects.enter("gfmFootnoteCallLabelMarker");
      effects.consume(code4);
      effects.exit("gfmFootnoteCallLabelMarker");
      effects.exit("gfmFootnoteCall");
      return ok3;
    }
    if (!markdownLineEndingOrSpace(code4)) {
      data = true;
    }
    size++;
    effects.consume(code4);
    return code4 === 92 ? callEscape : callData;
  }
  function callEscape(code4) {
    if (code4 === 91 || code4 === 92 || code4 === 93) {
      effects.consume(code4);
      size++;
      return callData;
    }
    return callData(code4);
  }
}
function tokenizeDefinitionStart(effects, ok3, nok) {
  const self2 = this;
  const defined = self2.parser.gfmFootnotes || (self2.parser.gfmFootnotes = []);
  let identifier;
  let size = 0;
  let data;
  return start2;
  function start2(code4) {
    effects.enter("gfmFootnoteDefinition")._container = true;
    effects.enter("gfmFootnoteDefinitionLabel");
    effects.enter("gfmFootnoteDefinitionLabelMarker");
    effects.consume(code4);
    effects.exit("gfmFootnoteDefinitionLabelMarker");
    return labelAtMarker;
  }
  function labelAtMarker(code4) {
    if (code4 === 94) {
      effects.enter("gfmFootnoteDefinitionMarker");
      effects.consume(code4);
      effects.exit("gfmFootnoteDefinitionMarker");
      effects.enter("gfmFootnoteDefinitionLabelString");
      effects.enter("chunkString").contentType = "string";
      return labelInside;
    }
    return nok(code4);
  }
  function labelInside(code4) {
    if (
      // Too long.
      size > 999 || // Closing brace with nothing.
      code4 === 93 && !data || // Space or tab is not supported by GFM for some reason.
      // `\n` and `[` not being supported makes sense.
      code4 === null || code4 === 91 || markdownLineEndingOrSpace(code4)
    ) {
      return nok(code4);
    }
    if (code4 === 93) {
      effects.exit("chunkString");
      const token = effects.exit("gfmFootnoteDefinitionLabelString");
      identifier = normalizeIdentifier(self2.sliceSerialize(token));
      effects.enter("gfmFootnoteDefinitionLabelMarker");
      effects.consume(code4);
      effects.exit("gfmFootnoteDefinitionLabelMarker");
      effects.exit("gfmFootnoteDefinitionLabel");
      return labelAfter;
    }
    if (!markdownLineEndingOrSpace(code4)) {
      data = true;
    }
    size++;
    effects.consume(code4);
    return code4 === 92 ? labelEscape : labelInside;
  }
  function labelEscape(code4) {
    if (code4 === 91 || code4 === 92 || code4 === 93) {
      effects.consume(code4);
      size++;
      return labelInside;
    }
    return labelInside(code4);
  }
  function labelAfter(code4) {
    if (code4 === 58) {
      effects.enter("definitionMarker");
      effects.consume(code4);
      effects.exit("definitionMarker");
      if (!defined.includes(identifier)) {
        defined.push(identifier);
      }
      return factorySpace(effects, whitespaceAfter, "gfmFootnoteDefinitionWhitespace");
    }
    return nok(code4);
  }
  function whitespaceAfter(code4) {
    return ok3(code4);
  }
}
function tokenizeDefinitionContinuation(effects, ok3, nok) {
  return effects.check(blankLine, ok3, effects.attempt(indent, ok3, nok));
}
function gfmFootnoteDefinitionEnd(effects) {
  effects.exit("gfmFootnoteDefinition");
}
function tokenizeIndent2(effects, ok3, nok) {
  const self2 = this;
  return factorySpace(effects, afterPrefix, "gfmFootnoteDefinitionIndent", 4 + 1);
  function afterPrefix(code4) {
    const tail = self2.events[self2.events.length - 1];
    return tail && tail[1].type === "gfmFootnoteDefinitionIndent" && tail[2].sliceSerialize(tail[1], true).length === 4 ? ok3(code4) : nok(code4);
  }
}

// node_modules/micromark-extension-gfm-strikethrough/lib/syntax.js
function gfmStrikethrough(options) {
  const options_ = options || {};
  let single = options_.singleTilde;
  const tokenizer = {
    name: "strikethrough",
    tokenize: tokenizeStrikethrough,
    resolveAll: resolveAllStrikethrough
  };
  if (single === null || single === void 0) {
    single = true;
  }
  return {
    text: {
      [126]: tokenizer
    },
    insideSpan: {
      null: [tokenizer]
    },
    attentionMarkers: {
      null: [126]
    }
  };
  function resolveAllStrikethrough(events, context) {
    let index3 = -1;
    while (++index3 < events.length) {
      if (events[index3][0] === "enter" && events[index3][1].type === "strikethroughSequenceTemporary" && events[index3][1]._close) {
        let open = index3;
        while (open--) {
          if (events[open][0] === "exit" && events[open][1].type === "strikethroughSequenceTemporary" && events[open][1]._open && // If the sizes are the same:
          events[index3][1].end.offset - events[index3][1].start.offset === events[open][1].end.offset - events[open][1].start.offset) {
            events[index3][1].type = "strikethroughSequence";
            events[open][1].type = "strikethroughSequence";
            const strikethrough2 = {
              type: "strikethrough",
              start: Object.assign({}, events[open][1].start),
              end: Object.assign({}, events[index3][1].end)
            };
            const text7 = {
              type: "strikethroughText",
              start: Object.assign({}, events[open][1].end),
              end: Object.assign({}, events[index3][1].start)
            };
            const nextEvents = [["enter", strikethrough2, context], ["enter", events[open][1], context], ["exit", events[open][1], context], ["enter", text7, context]];
            const insideSpan2 = context.parser.constructs.insideSpan.null;
            if (insideSpan2) {
              splice(nextEvents, nextEvents.length, 0, resolveAll(insideSpan2, events.slice(open + 1, index3), context));
            }
            splice(nextEvents, nextEvents.length, 0, [["exit", text7, context], ["enter", events[index3][1], context], ["exit", events[index3][1], context], ["exit", strikethrough2, context]]);
            splice(events, open - 1, index3 - open + 3, nextEvents);
            index3 = open + nextEvents.length - 2;
            break;
          }
        }
      }
    }
    index3 = -1;
    while (++index3 < events.length) {
      if (events[index3][1].type === "strikethroughSequenceTemporary") {
        events[index3][1].type = "data";
      }
    }
    return events;
  }
  function tokenizeStrikethrough(effects, ok3, nok) {
    const previous3 = this.previous;
    const events = this.events;
    let size = 0;
    return start2;
    function start2(code4) {
      if (previous3 === 126 && events[events.length - 1][1].type !== "characterEscape") {
        return nok(code4);
      }
      effects.enter("strikethroughSequenceTemporary");
      return more(code4);
    }
    function more(code4) {
      const before = classifyCharacter(previous3);
      if (code4 === 126) {
        if (size > 1) return nok(code4);
        effects.consume(code4);
        size++;
        return more;
      }
      if (size < 2 && !single) return nok(code4);
      const token = effects.exit("strikethroughSequenceTemporary");
      const after = classifyCharacter(code4);
      token._open = !after || after === 2 && Boolean(before);
      token._close = !before || before === 2 && Boolean(after);
      return ok3(code4);
    }
  }
}

// node_modules/micromark-extension-gfm-table/lib/edit-map.js
var EditMap2 = class {
  /**
   * Create a new edit map.
   */
  constructor() {
    this.map = [];
    this.index = /* @__PURE__ */ new Map();
  }
  /**
   * Create an edit: a remove and/or add at a certain place.
   *
   * @param {number} index
   *   Index at which to apply the edit.
   * @param {number} remove
   *   Count of items to remove at the index.
   * @param {Array<Event>} add
   *   Items to add at the index.
   * @returns {undefined}
   *   Nothing.
   */
  add(index3, remove, add) {
    addImplementation2(this, index3, remove, add);
  }
  // To do: add this when moving to `micromark`.
  // /**
  //  * Create an edit: but insert `add` before existing additions.
  //  *
  //  * @param {number} index
  //  * @param {number} remove
  //  * @param {Array<Event>} add
  //  * @returns {undefined}
  //  */
  // addBefore(index, remove, add) {
  //   addImplementation(this, index, remove, add, true)
  // }
  /**
   * Done, change the events.
   *
   * @param {Array<Event>} events
   *   List of events to apply the edits to.
   * @returns {undefined}
   *   Nothing.
   */
  consume(events) {
    this.map.sort(function(a, b) {
      return a[0] - b[0];
    });
    if (this.map.length === 0) {
      return;
    }
    let index3 = this.map.length;
    const vecs = [];
    while (index3 > 0) {
      index3 -= 1;
      vecs.push(events.slice(this.map[index3][0] + this.map[index3][1]), this.map[index3][2]);
      events.length = this.map[index3][0];
    }
    vecs.push(events.slice());
    events.length = 0;
    let slice = vecs.pop();
    while (slice) {
      for (const element3 of slice) {
        events.push(element3);
      }
      slice = vecs.pop();
    }
    this.map.length = 0;
    this.index.clear();
  }
};
function addImplementation2(editMap, at, remove, add) {
  if (remove === 0 && add.length === 0) {
    return;
  }
  const existing = editMap.index.get(at);
  if (existing) {
    existing[1] += remove;
    existing[2].push(...add);
    return;
  }
  const change = [at, remove, add];
  editMap.map.push(change);
  editMap.index.set(at, change);
}

// node_modules/micromark-extension-gfm-table/lib/infer.js
function gfmTableAlign(events, index3) {
  let inDelimiterRow = false;
  const align = [];
  while (index3 < events.length) {
    const event = events[index3];
    if (inDelimiterRow) {
      if (event[0] === "enter") {
        if (event[1].type === "tableContent") {
          align.push(events[index3 + 1][1].type === "tableDelimiterMarker" ? "left" : "none");
        }
      } else if (event[1].type === "tableContent") {
        if (events[index3 - 1][1].type === "tableDelimiterMarker") {
          const alignIndex = align.length - 1;
          align[alignIndex] = align[alignIndex] === "left" ? "center" : "right";
        }
      } else if (event[1].type === "tableDelimiterRow") {
        break;
      }
    } else if (event[0] === "enter" && event[1].type === "tableDelimiterRow") {
      inDelimiterRow = true;
    }
    index3 += 1;
  }
  return align;
}

// node_modules/micromark-extension-gfm-table/lib/syntax.js
function gfmTable() {
  return {
    flow: {
      null: {
        name: "table",
        tokenize: tokenizeTable,
        resolveAll: resolveTable
      }
    }
  };
}
function tokenizeTable(effects, ok3, nok) {
  const self2 = this;
  let size = 0;
  let sizeB = 0;
  let seen;
  return start2;
  function start2(code4) {
    let index3 = self2.events.length - 1;
    while (index3 > -1) {
      const {
        type
      } = self2.events[index3][1];
      if (type === "lineEnding" || // Note: markdown-rs uses `whitespace` instead of `linePrefix`
      type === "linePrefix") {
        index3--;
      } else {
        break;
      }
    }
    const tail = index3 > -1 ? self2.events[index3][1].type : null;
    const next = tail === "tableHead" || tail === "tableRow" ? bodyRowStart : headRowBefore;
    if (next === bodyRowStart && self2.parser.lazy[self2.now().line]) {
      return nok(code4);
    }
    return next(code4);
  }
  function headRowBefore(code4) {
    effects.enter("tableHead");
    effects.enter("tableRow");
    return headRowStart(code4);
  }
  function headRowStart(code4) {
    if (code4 === 124) {
      return headRowBreak(code4);
    }
    seen = true;
    sizeB += 1;
    return headRowBreak(code4);
  }
  function headRowBreak(code4) {
    if (code4 === null) {
      return nok(code4);
    }
    if (markdownLineEnding(code4)) {
      if (sizeB > 1) {
        sizeB = 0;
        self2.interrupt = true;
        effects.exit("tableRow");
        effects.enter("lineEnding");
        effects.consume(code4);
        effects.exit("lineEnding");
        return headDelimiterStart;
      }
      return nok(code4);
    }
    if (markdownSpace(code4)) {
      return factorySpace(effects, headRowBreak, "whitespace")(code4);
    }
    sizeB += 1;
    if (seen) {
      seen = false;
      size += 1;
    }
    if (code4 === 124) {
      effects.enter("tableCellDivider");
      effects.consume(code4);
      effects.exit("tableCellDivider");
      seen = true;
      return headRowBreak;
    }
    effects.enter("data");
    return headRowData(code4);
  }
  function headRowData(code4) {
    if (code4 === null || code4 === 124 || markdownLineEndingOrSpace(code4)) {
      effects.exit("data");
      return headRowBreak(code4);
    }
    effects.consume(code4);
    return code4 === 92 ? headRowEscape : headRowData;
  }
  function headRowEscape(code4) {
    if (code4 === 92 || code4 === 124) {
      effects.consume(code4);
      return headRowData;
    }
    return headRowData(code4);
  }
  function headDelimiterStart(code4) {
    self2.interrupt = false;
    if (self2.parser.lazy[self2.now().line]) {
      return nok(code4);
    }
    effects.enter("tableDelimiterRow");
    seen = false;
    if (markdownSpace(code4)) {
      return factorySpace(effects, headDelimiterBefore, "linePrefix", self2.parser.constructs.disable.null.includes("codeIndented") ? void 0 : 4)(code4);
    }
    return headDelimiterBefore(code4);
  }
  function headDelimiterBefore(code4) {
    if (code4 === 45 || code4 === 58) {
      return headDelimiterValueBefore(code4);
    }
    if (code4 === 124) {
      seen = true;
      effects.enter("tableCellDivider");
      effects.consume(code4);
      effects.exit("tableCellDivider");
      return headDelimiterCellBefore;
    }
    return headDelimiterNok(code4);
  }
  function headDelimiterCellBefore(code4) {
    if (markdownSpace(code4)) {
      return factorySpace(effects, headDelimiterValueBefore, "whitespace")(code4);
    }
    return headDelimiterValueBefore(code4);
  }
  function headDelimiterValueBefore(code4) {
    if (code4 === 58) {
      sizeB += 1;
      seen = true;
      effects.enter("tableDelimiterMarker");
      effects.consume(code4);
      effects.exit("tableDelimiterMarker");
      return headDelimiterLeftAlignmentAfter;
    }
    if (code4 === 45) {
      sizeB += 1;
      return headDelimiterLeftAlignmentAfter(code4);
    }
    if (code4 === null || markdownLineEnding(code4)) {
      return headDelimiterCellAfter(code4);
    }
    return headDelimiterNok(code4);
  }
  function headDelimiterLeftAlignmentAfter(code4) {
    if (code4 === 45) {
      effects.enter("tableDelimiterFiller");
      return headDelimiterFiller(code4);
    }
    return headDelimiterNok(code4);
  }
  function headDelimiterFiller(code4) {
    if (code4 === 45) {
      effects.consume(code4);
      return headDelimiterFiller;
    }
    if (code4 === 58) {
      seen = true;
      effects.exit("tableDelimiterFiller");
      effects.enter("tableDelimiterMarker");
      effects.consume(code4);
      effects.exit("tableDelimiterMarker");
      return headDelimiterRightAlignmentAfter;
    }
    effects.exit("tableDelimiterFiller");
    return headDelimiterRightAlignmentAfter(code4);
  }
  function headDelimiterRightAlignmentAfter(code4) {
    if (markdownSpace(code4)) {
      return factorySpace(effects, headDelimiterCellAfter, "whitespace")(code4);
    }
    return headDelimiterCellAfter(code4);
  }
  function headDelimiterCellAfter(code4) {
    if (code4 === 124) {
      return headDelimiterBefore(code4);
    }
    if (code4 === null || markdownLineEnding(code4)) {
      if (!seen || size !== sizeB) {
        return headDelimiterNok(code4);
      }
      effects.exit("tableDelimiterRow");
      effects.exit("tableHead");
      return ok3(code4);
    }
    return headDelimiterNok(code4);
  }
  function headDelimiterNok(code4) {
    return nok(code4);
  }
  function bodyRowStart(code4) {
    effects.enter("tableRow");
    return bodyRowBreak(code4);
  }
  function bodyRowBreak(code4) {
    if (code4 === 124) {
      effects.enter("tableCellDivider");
      effects.consume(code4);
      effects.exit("tableCellDivider");
      return bodyRowBreak;
    }
    if (code4 === null || markdownLineEnding(code4)) {
      effects.exit("tableRow");
      return ok3(code4);
    }
    if (markdownSpace(code4)) {
      return factorySpace(effects, bodyRowBreak, "whitespace")(code4);
    }
    effects.enter("data");
    return bodyRowData(code4);
  }
  function bodyRowData(code4) {
    if (code4 === null || code4 === 124 || markdownLineEndingOrSpace(code4)) {
      effects.exit("data");
      return bodyRowBreak(code4);
    }
    effects.consume(code4);
    return code4 === 92 ? bodyRowEscape : bodyRowData;
  }
  function bodyRowEscape(code4) {
    if (code4 === 92 || code4 === 124) {
      effects.consume(code4);
      return bodyRowData;
    }
    return bodyRowData(code4);
  }
}
function resolveTable(events, context) {
  let index3 = -1;
  let inFirstCellAwaitingPipe = true;
  let rowKind = 0;
  let lastCell = [0, 0, 0, 0];
  let cell = [0, 0, 0, 0];
  let afterHeadAwaitingFirstBodyRow = false;
  let lastTableEnd = 0;
  let currentTable;
  let currentBody;
  let currentCell;
  const map3 = new EditMap2();
  while (++index3 < events.length) {
    const event = events[index3];
    const token = event[1];
    if (event[0] === "enter") {
      if (token.type === "tableHead") {
        afterHeadAwaitingFirstBodyRow = false;
        if (lastTableEnd !== 0) {
          flushTableEnd(map3, context, lastTableEnd, currentTable, currentBody);
          currentBody = void 0;
          lastTableEnd = 0;
        }
        currentTable = {
          type: "table",
          start: Object.assign({}, token.start),
          // Note: correct end is set later.
          end: Object.assign({}, token.end)
        };
        map3.add(index3, 0, [["enter", currentTable, context]]);
      } else if (token.type === "tableRow" || token.type === "tableDelimiterRow") {
        inFirstCellAwaitingPipe = true;
        currentCell = void 0;
        lastCell = [0, 0, 0, 0];
        cell = [0, index3 + 1, 0, 0];
        if (afterHeadAwaitingFirstBodyRow) {
          afterHeadAwaitingFirstBodyRow = false;
          currentBody = {
            type: "tableBody",
            start: Object.assign({}, token.start),
            // Note: correct end is set later.
            end: Object.assign({}, token.end)
          };
          map3.add(index3, 0, [["enter", currentBody, context]]);
        }
        rowKind = token.type === "tableDelimiterRow" ? 2 : currentBody ? 3 : 1;
      } else if (rowKind && (token.type === "data" || token.type === "tableDelimiterMarker" || token.type === "tableDelimiterFiller")) {
        inFirstCellAwaitingPipe = false;
        if (cell[2] === 0) {
          if (lastCell[1] !== 0) {
            cell[0] = cell[1];
            currentCell = flushCell(map3, context, lastCell, rowKind, void 0, currentCell);
            lastCell = [0, 0, 0, 0];
          }
          cell[2] = index3;
        }
      } else if (token.type === "tableCellDivider") {
        if (inFirstCellAwaitingPipe) {
          inFirstCellAwaitingPipe = false;
        } else {
          if (lastCell[1] !== 0) {
            cell[0] = cell[1];
            currentCell = flushCell(map3, context, lastCell, rowKind, void 0, currentCell);
          }
          lastCell = cell;
          cell = [lastCell[1], index3, 0, 0];
        }
      }
    } else if (token.type === "tableHead") {
      afterHeadAwaitingFirstBodyRow = true;
      lastTableEnd = index3;
    } else if (token.type === "tableRow" || token.type === "tableDelimiterRow") {
      lastTableEnd = index3;
      if (lastCell[1] !== 0) {
        cell[0] = cell[1];
        currentCell = flushCell(map3, context, lastCell, rowKind, index3, currentCell);
      } else if (cell[1] !== 0) {
        currentCell = flushCell(map3, context, cell, rowKind, index3, currentCell);
      }
      rowKind = 0;
    } else if (rowKind && (token.type === "data" || token.type === "tableDelimiterMarker" || token.type === "tableDelimiterFiller")) {
      cell[3] = index3;
    }
  }
  if (lastTableEnd !== 0) {
    flushTableEnd(map3, context, lastTableEnd, currentTable, currentBody);
  }
  map3.consume(context.events);
  index3 = -1;
  while (++index3 < context.events.length) {
    const event = context.events[index3];
    if (event[0] === "enter" && event[1].type === "table") {
      event[1]._align = gfmTableAlign(context.events, index3);
    }
  }
  return events;
}
function flushCell(map3, context, range, rowKind, rowEnd, previousCell) {
  const groupName = rowKind === 1 ? "tableHeader" : rowKind === 2 ? "tableDelimiter" : "tableData";
  const valueName = "tableContent";
  if (range[0] !== 0) {
    previousCell.end = Object.assign({}, getPoint(context.events, range[0]));
    map3.add(range[0], 0, [["exit", previousCell, context]]);
  }
  const now = getPoint(context.events, range[1]);
  previousCell = {
    type: groupName,
    start: Object.assign({}, now),
    // Note: correct end is set later.
    end: Object.assign({}, now)
  };
  map3.add(range[1], 0, [["enter", previousCell, context]]);
  if (range[2] !== 0) {
    const relatedStart = getPoint(context.events, range[2]);
    const relatedEnd = getPoint(context.events, range[3]);
    const valueToken = {
      type: valueName,
      start: Object.assign({}, relatedStart),
      end: Object.assign({}, relatedEnd)
    };
    map3.add(range[2], 0, [["enter", valueToken, context]]);
    if (rowKind !== 2) {
      const start2 = context.events[range[2]];
      const end = context.events[range[3]];
      start2[1].end = Object.assign({}, end[1].end);
      start2[1].type = "chunkText";
      start2[1].contentType = "text";
      if (range[3] > range[2] + 1) {
        const a = range[2] + 1;
        const b = range[3] - range[2] - 1;
        map3.add(a, b, []);
      }
    }
    map3.add(range[3] + 1, 0, [["exit", valueToken, context]]);
  }
  if (rowEnd !== void 0) {
    previousCell.end = Object.assign({}, getPoint(context.events, rowEnd));
    map3.add(rowEnd, 0, [["exit", previousCell, context]]);
    previousCell = void 0;
  }
  return previousCell;
}
function flushTableEnd(map3, context, index3, table2, tableBody) {
  const exits = [];
  const related = getPoint(context.events, index3);
  if (tableBody) {
    tableBody.end = Object.assign({}, related);
    exits.push(["exit", tableBody, context]);
  }
  table2.end = Object.assign({}, related);
  exits.push(["exit", table2, context]);
  map3.add(index3 + 1, 0, exits);
}
function getPoint(events, index3) {
  const event = events[index3];
  const side = event[0] === "enter" ? "start" : "end";
  return event[1][side];
}

// node_modules/micromark-extension-gfm-task-list-item/lib/syntax.js
var tasklistCheck = {
  name: "tasklistCheck",
  tokenize: tokenizeTasklistCheck
};
function gfmTaskListItem() {
  return {
    text: {
      [91]: tasklistCheck
    }
  };
}
function tokenizeTasklistCheck(effects, ok3, nok) {
  const self2 = this;
  return open;
  function open(code4) {
    if (
      // Exit if there’s stuff before.
      self2.previous !== null || // Exit if not in the first content that is the first child of a list
      // item.
      !self2._gfmTasklistFirstContentOfListItem
    ) {
      return nok(code4);
    }
    effects.enter("taskListCheck");
    effects.enter("taskListCheckMarker");
    effects.consume(code4);
    effects.exit("taskListCheckMarker");
    return inside;
  }
  function inside(code4) {
    if (markdownLineEndingOrSpace(code4)) {
      effects.enter("taskListCheckValueUnchecked");
      effects.consume(code4);
      effects.exit("taskListCheckValueUnchecked");
      return close;
    }
    if (code4 === 88 || code4 === 120) {
      effects.enter("taskListCheckValueChecked");
      effects.consume(code4);
      effects.exit("taskListCheckValueChecked");
      return close;
    }
    return nok(code4);
  }
  function close(code4) {
    if (code4 === 93) {
      effects.enter("taskListCheckMarker");
      effects.consume(code4);
      effects.exit("taskListCheckMarker");
      effects.exit("taskListCheck");
      return after;
    }
    return nok(code4);
  }
  function after(code4) {
    if (markdownLineEnding(code4)) {
      return ok3(code4);
    }
    if (markdownSpace(code4)) {
      return effects.check({
        tokenize: spaceThenNonSpace
      }, ok3, nok)(code4);
    }
    return nok(code4);
  }
}
function spaceThenNonSpace(effects, ok3, nok) {
  return factorySpace(effects, after, "whitespace");
  function after(code4) {
    return code4 === null ? nok(code4) : ok3(code4);
  }
}

// node_modules/micromark-extension-gfm/index.js
function gfm(options) {
  return combineExtensions([
    gfmAutolinkLiteral(),
    gfmFootnote(),
    gfmStrikethrough(options),
    gfmTable(),
    gfmTaskListItem()
  ]);
}

// node_modules/remark-gfm/lib/index.js
var emptyOptions4 = {};
function remarkGfm(options) {
  const self2 = (
    /** @type {Processor<Root>} */
    this
  );
  const settings = options || emptyOptions4;
  const data = self2.data();
  const micromarkExtensions = data.micromarkExtensions || (data.micromarkExtensions = []);
  const fromMarkdownExtensions = data.fromMarkdownExtensions || (data.fromMarkdownExtensions = []);
  const toMarkdownExtensions = data.toMarkdownExtensions || (data.toMarkdownExtensions = []);
  micromarkExtensions.push(gfm(settings));
  fromMarkdownExtensions.push(gfmFromMarkdown());
  toMarkdownExtensions.push(gfmToMarkdown(settings));
}

// src/items.tsx
import { Fragment as Fragment4, jsx as jsx4, jsxs as jsxs3 } from "react/jsx-runtime";
var ICON = {
  // 用向下 chevron 表示收起，展开时旋转 180°。
  chevron: "M6 9l6 6 6-6",
  copy: "M9 9h10v11H9z M5 15V5a1 1 0 0 1 1-1h9",
  file: "M6 3h8l4 4v14H6z M14 3v4h4",
  // 用数据库图标（IconDatabaseOutlineRegular）标记 token 用量。
  db: "M12 2C7.58 2 4 3.34 4 5s3.58 3 8 3 8-1.34 8-3-3.58-3-8-3z M4 5v14c0 1.66 3.58 3 8 3s8-1.34 8-3V5 M4 12c0 1.66 3.58 3 8 3s8-1.34 8-3",
  // 命令/压缩行用 IconApiOutlineRegular（方框内 `>_` 终端提示符）。
  terminal: "M4 5h16v14H4z M8 10l2 2-2 2 M13 14h3"
};
var TOOL_LABELS = {
  read: "\u8BFB\u53D6",
  read_image: "\u8BFB\u53D6\u56FE\u7247",
  write: "\u5199\u5165",
  edit: "\u7F16\u8F91",
  bash: "\u8FD0\u884C\u547D\u4EE4",
  glob: "\u67E5\u627E\u6587\u4EF6",
  grep: "\u641C\u7D22\u6587\u4EF6\u5185\u5BB9",
  web_search: "\u7F51\u9875\u641C\u7D22",
  web_fetch: "\u7F51\u9875\u83B7\u53D6",
  skill: "\u52A0\u8F7D\u6280\u80FD",
  task: "\u5B50\u4EFB\u52A1",
  present: "\u5C55\u793A\u6587\u4EF6",
  todo_write: "\u66F4\u65B0\u4EFB\u52A1\u6E05\u5355"
};
function toolLabel(name2) {
  return TOOL_LABELS[name2.toLowerCase()] ?? name2;
}
function parsePresented(text7) {
  if (!text7) return [];
  try {
    const parsed = JSON.parse(text7);
    const files = Array.isArray(parsed.files) ? parsed.files : [];
    return files.flatMap((raw) => {
      const file = typeof raw === "object" && raw !== null ? raw : {};
      if (typeof file.path !== "string" || !file.path) return [];
      return [
        {
          path: file.path,
          description: typeof file.description === "string" ? file.description : void 0
        }
      ];
    });
  } catch {
    return [];
  }
}
function baseName(path2) {
  const parts = path2.split(/[\\/]/).filter(Boolean);
  return parts.length > 0 ? parts[parts.length - 1] : path2;
}
function Icon({ d, size = 14 }) {
  return /* @__PURE__ */ jsx4(
    "svg",
    {
      width: size,
      height: size,
      viewBox: "0 0 24 24",
      fill: "none",
      stroke: "currentColor",
      strokeWidth: "1.6",
      strokeLinecap: "round",
      strokeLinejoin: "round",
      "aria-hidden": "true",
      children: /* @__PURE__ */ jsx4("path", { d })
    }
  );
}
function useCopy() {
  const [copied, setCopied] = useState4(false);
  const copy = (text7) => {
    navigator.clipboard?.writeText(text7).then(() => {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    }).catch(() => void 0);
  };
  return [copied, copy];
}
function itemAnchor(index3) {
  return `cc-item-${index3}`;
}
function Markdown2({ text: text7 }) {
  return /* @__PURE__ */ jsx4("div", { className: "cc-md", children: /* @__PURE__ */ jsx4(Markdown, { remarkPlugins: [remarkGfm], components: { pre: CodeBlock }, children: text7 }) });
}
function CodeBlock({ children }) {
  const child = Array.isArray(children) ? children[0] : children;
  const props = child?.props;
  const lang = /language-([\w-]+)/.exec(props?.className ?? "")?.[1] ?? "text";
  const code4 = String(props?.children ?? "").replace(/\n$/, "");
  const [copied, copy] = useCopy();
  return /* @__PURE__ */ jsxs3("div", { className: "cc-code", children: [
    /* @__PURE__ */ jsxs3("div", { className: "cc-code-bar", children: [
      /* @__PURE__ */ jsx4("span", { children: lang }),
      /* @__PURE__ */ jsx4("button", { className: "cc-code-copy", onClick: () => copy(code4), children: copied ? "\u5DF2\u590D\u5236" : "\u590D\u5236" })
    ] }),
    /* @__PURE__ */ jsx4("pre", { children: /* @__PURE__ */ jsx4("code", { className: props?.className, children: code4 }) })
  ] });
}
function StateDot({ step }) {
  if (step.running) return /* @__PURE__ */ jsx4("span", { className: "cc-state-dot running", "aria-label": "\u8FD0\u884C\u4E2D" });
  if (step.isError) return /* @__PURE__ */ jsx4("span", { className: "cc-state-dot err", "aria-label": "\u5931\u8D25" });
  return /* @__PURE__ */ jsx4("span", { className: "cc-state-dot ok", "aria-label": "\u5B8C\u6210" });
}
function todoSummary(inputText) {
  if (!inputText) return null;
  try {
    const parsed = JSON.parse(inputText);
    const list4 = Array.isArray(parsed.todos) ? parsed.todos : [];
    const items = list4.flatMap((raw) => {
      const t = typeof raw === "object" && raw !== null ? raw : {};
      if (typeof t.content !== "string" || !t.content) return [];
      return [{ content: t.content, status: String(t.status ?? "pending") }];
    });
    if (items.length === 0) return null;
    const done = items.filter((t) => t.status === "completed").length;
    const active = items.filter((t) => t.status === "in_progress");
    const head = `${done}/${items.length} \u5DF2\u5B8C\u6210`;
    if (active.length === 0) return { text: head, extra: 0 };
    return { text: `${head} \xB7 ${firstLine(active[0].content, 48)}`, extra: active.length - 1 };
  } catch {
    return null;
  }
}
function ToolRow({ step }) {
  const [open, setOpen] = useState4(false);
  const isPresent = step.name.toLowerCase() === "present";
  const hasDetail = Boolean(step.inputText || step.resultText && !isPresent);
  const todo = step.name.toLowerCase() === "todo_write" ? todoSummary(step.inputText ?? step.resultText) : null;
  const summary = step.running ? "\u8FD0\u884C\u4E2D\u2026" : todo ? todo.text : firstLine(step.isError ? step.resultText ?? "" : step.inputText ?? "", 72);
  return /* @__PURE__ */ jsxs3("div", { className: "cc-tool", children: [
    /* @__PURE__ */ jsxs3(
      "button",
      {
        className: "cc-row" + (hasDetail ? "" : " cc-row-static"),
        "aria-expanded": hasDetail ? open : void 0,
        onClick: () => hasDetail && setOpen((v) => !v),
        children: [
          /* @__PURE__ */ jsx4(StateDot, { step }),
          /* @__PURE__ */ jsx4("span", { className: "cc-tool-name", children: toolLabel(step.name) }),
          summary && /* @__PURE__ */ jsxs3(Fragment4, { children: [
            /* @__PURE__ */ jsx4("span", { className: "cc-tool-sep", "aria-hidden": "true" }),
            /* @__PURE__ */ jsx4("span", { className: "cc-row-summary" + (step.isError ? " err" : ""), children: summary }),
            todo && todo.extra > 0 && /* @__PURE__ */ jsxs3("span", { className: "cc-tool-extra", children: [
              "+",
              todo.extra
            ] })
          ] })
        ]
      }
    ),
    open && hasDetail && /* @__PURE__ */ jsxs3("div", { className: "cc-tool-details", children: [
      step.inputText && /* @__PURE__ */ jsxs3("div", { className: "cc-tool-detail-row", children: [
        /* @__PURE__ */ jsx4("span", { className: "cc-tool-detail-label", children: "\u8F93\u5165" }),
        /* @__PURE__ */ jsx4("pre", { children: step.inputText })
      ] }),
      step.resultText && !isPresent && /* @__PURE__ */ jsxs3("div", { className: "cc-tool-detail-row" + (step.isError ? " err" : ""), children: [
        /* @__PURE__ */ jsx4("span", { className: "cc-tool-detail-label", children: "\u7ED3\u679C" }),
        /* @__PURE__ */ jsx4("pre", { children: step.resultText })
      ] })
    ] })
  ] });
}
function presentedOf(item) {
  const byPath = /* @__PURE__ */ new Map();
  for (const step of item.steps) {
    if (step.name.toLowerCase() !== "present" || step.isError) continue;
    for (const file of parsePresented(step.resultText)) byPath.set(file.path, file);
  }
  return [...byPath.values()];
}
function Deliverables({ files, sessionId }) {
  return /* @__PURE__ */ jsx4("div", { className: "cc-deliverables", children: files.map((file) => /* @__PURE__ */ jsxs3("div", { className: "cc-deliverable", children: [
    /* @__PURE__ */ jsx4("span", { className: "cc-deliverable-icon", children: /* @__PURE__ */ jsx4(Icon, { d: ICON.file }) }),
    /* @__PURE__ */ jsxs3("span", { className: "cc-deliverable-body", children: [
      /* @__PURE__ */ jsx4("span", { className: "cc-deliverable-name", title: file.path, children: baseName(file.path) }),
      file.description && /* @__PURE__ */ jsx4("span", { className: "cc-deliverable-desc", children: file.description })
    ] }),
    /* @__PURE__ */ jsx4(
      "button",
      {
        className: "cc-deliverable-open",
        onClick: () => openInCanvas(file.path, sessionId),
        title: file.path,
        children: "\u5728\u753B\u5E03\u6253\u5F00"
      }
    )
  ] }, file.path)) });
}
function firstLineOf(text7) {
  const newline = text7.indexOf("\n");
  return newline === -1 ? text7 : text7.slice(0, newline);
}
function latestParagraphFirstLine(text7) {
  let summary = "";
  let paragraphStart = 0;
  const separator = /\r?\n(?:[\t ]*\r?\n)+/g;
  while (true) {
    const nextParagraph = separator.exec(text7);
    const paragraphEnd = nextParagraph === null ? text7.length : nextParagraph.index + nextParagraph[0].indexOf("\n");
    const newline = text7.indexOf("\n", paragraphStart);
    if (newline !== -1 && newline <= paragraphEnd) {
      const candidate = text7.slice(paragraphStart, newline).trim();
      if (candidate !== "") summary = candidate;
    }
    if (nextParagraph === null) return summary;
    paragraphStart = nextParagraph.index + nextParagraph[0].length;
  }
}
function ReasoningRow({ thinking, live }) {
  const [open, setOpen] = useState4(false);
  const summary = (live && !open ? latestParagraphFirstLine(thinking) : firstLineOf(thinking)).replaceAll(
    "**",
    ""
  );
  return /* @__PURE__ */ jsxs3(Fragment4, { children: [
    /* @__PURE__ */ jsxs3("button", { className: "cc-row", "aria-expanded": open, onClick: () => setOpen((v) => !v), children: [
      /* @__PURE__ */ jsx4("span", { className: "cc-chevron" + (open ? " open" : ""), children: /* @__PURE__ */ jsx4(Icon, { d: ICON.chevron }) }),
      /* @__PURE__ */ jsx4("span", { className: "cc-tool-name", children: live ? "\u601D\u8003\u4E2D" : "\u601D\u8003" }),
      !open && /* @__PURE__ */ jsx4("span", { className: "cc-row-summary" + (live ? " cc-shimmer" : ""), children: summary })
    ] }),
    open && /* @__PURE__ */ jsx4("div", { className: "cc-thinking", children: /* @__PURE__ */ jsx4(Markdown2, { text: thinking }) })
  ] });
}
function ProcessBlock({ item }) {
  const [open, setOpen] = useState4(false);
  const live = item.streaming;
  const steps = item.steps.length;
  const reason = item.report?.reason ?? "";
  const meta = live ? "\u5904\u7406\u4E2D\u2026" : [
    reason === "cancelled" ? "\u5DF2\u53D6\u6D88" : "\u5DF2\u5B8C\u6210",
    steps > 0 ? `${steps} \u6B65` : "",
    reason === "cancelled" ? "" : reason
  ].filter(Boolean).join(" \xB7 ");
  const hasThinking = item.thinkingSegs.some((s) => s.trim() !== "");
  const empty3 = !hasThinking && steps === 0;
  const tailSeg = item.thinkingSegs[steps] ?? "";
  return /* @__PURE__ */ jsxs3("div", { children: [
    /* @__PURE__ */ jsxs3(
      "button",
      {
        className: "cc-process-head",
        "aria-expanded": open,
        onClick: () => setOpen((v) => !v),
        disabled: empty3,
        children: [
          /* @__PURE__ */ jsx4("span", { className: "cc-chevron" + (open ? " open" : ""), children: /* @__PURE__ */ jsx4(Icon, { d: ICON.chevron }) }),
          /* @__PURE__ */ jsx4("span", { className: "cc-process-meta", children: meta })
        ]
      }
    ),
    open && /* @__PURE__ */ jsxs3("div", { className: "cc-process-body", children: [
      item.steps.map((s, i) => {
        const seg = item.thinkingSegs[i] ?? "";
        return /* @__PURE__ */ jsxs3(Fragment3, { children: [
          seg.trim() !== "" && /* @__PURE__ */ jsx4(ReasoningRow, { thinking: seg, live: false }),
          /* @__PURE__ */ jsx4(ToolRow, { step: s })
        ] }, s.id || `${i}-${s.name}`);
      }),
      tailSeg.trim() !== "" && /* @__PURE__ */ jsx4(ReasoningRow, { thinking: tailSeg, live: live && !item.answer })
    ] })
  ] });
}
function UsagePill({ usage }) {
  const [open, setOpen] = useState4(false);
  const [anchor, setAnchor] = useState4(null);
  const total = usageTotal(usage);
  const uncached = Math.max(0, usage.inputTokens - usage.cacheReadTokens - usage.cacheWriteTokens);
  const cacheHit = formatCacheHitPercent(usage.cacheReadTokens, usage.inputTokens);
  return /* @__PURE__ */ jsxs3(Fragment4, { children: [
    /* @__PURE__ */ jsxs3(
      "button",
      {
        ref: setAnchor,
        className: "cc-turn-usage" + (open ? " open" : ""),
        "aria-haspopup": "dialog",
        "aria-expanded": open,
        onClick: () => setOpen((v) => !v),
        children: [
          /* @__PURE__ */ jsx4(Icon, { d: ICON.db, size: 15 }),
          /* @__PURE__ */ jsxs3("span", { className: "cc-turn-usage-label", children: [
            "\u7528\u91CF ",
            formatTokens(total),
            " tok"
          ] })
        ]
      }
    ),
    /* @__PURE__ */ jsx4(Popover, { anchor, open, onClose: () => setOpen(false), side: "top", width: 300, children: /* @__PURE__ */ jsxs3("div", { className: "cc-usage-pop", children: [
      /* @__PURE__ */ jsxs3("div", { className: "cc-usage-pop-title", children: [
        /* @__PURE__ */ jsxs3("span", { className: "cc-usage-pop-name", children: [
          /* @__PURE__ */ jsx4(Icon, { d: ICON.db, size: 14 }),
          "\u672C\u8F6E\u7528\u91CF"
        ] }),
        /* @__PURE__ */ jsx4("span", { className: "cc-usage-pop-total", children: formatExactTokens(total) })
      ] }),
      /* @__PURE__ */ jsx4("div", { className: "cc-usage-pop-rule", "aria-hidden": true }),
      /* @__PURE__ */ jsxs3("dl", { className: "cc-usage-pop-details", children: [
        cacheHit !== null && /* @__PURE__ */ jsxs3(Fragment4, { children: [
          /* @__PURE__ */ jsx4("dt", { children: "\u7F13\u5B58\u547D\u4E2D" }),
          /* @__PURE__ */ jsxs3("dd", { children: [
            cacheHit,
            "%"
          ] })
        ] }),
        /* @__PURE__ */ jsx4("dt", { children: "\u672A\u7F13\u5B58\u8F93\u5165" }),
        /* @__PURE__ */ jsx4("dd", { children: formatExactTokens(uncached) }),
        usage.cacheReadTokens > 0 && /* @__PURE__ */ jsxs3(Fragment4, { children: [
          /* @__PURE__ */ jsx4("dt", { children: "\u7F13\u5B58\u8BFB\u53D6" }),
          /* @__PURE__ */ jsx4("dd", { children: formatExactTokens(usage.cacheReadTokens) })
        ] }),
        usage.cacheWriteTokens > 0 && /* @__PURE__ */ jsxs3(Fragment4, { children: [
          /* @__PURE__ */ jsx4("dt", { children: "\u7F13\u5B58\u5199\u5165" }),
          /* @__PURE__ */ jsx4("dd", { children: formatExactTokens(usage.cacheWriteTokens) })
        ] }),
        /* @__PURE__ */ jsx4("dt", { children: "\u8F93\u51FA" }),
        /* @__PURE__ */ jsx4("dd", { children: formatExactTokens(usage.outputTokens) })
      ] })
    ] }) })
  ] });
}
function AssistantTurn({
  item,
  id,
  sessionId
}) {
  const [copied, copy] = useCopy();
  const usage = turnUsageOf(item);
  const tokens = usageTotal(usage);
  const hasProcess = item.thinkingSegs.some((s) => s.trim() !== "") || item.steps.length > 0 || tokens > 0;
  const deliverables = presentedOf(item);
  return /* @__PURE__ */ jsxs3("div", { className: "cc-assistant", id, children: [
    item.retryText && /* @__PURE__ */ jsxs3("div", { className: "cc-note", children: [
      "\u21BB ",
      truncate(item.retryText, 160)
    ] }),
    hasProcess && /* @__PURE__ */ jsx4(ProcessBlock, { item }),
    item.answer && /* @__PURE__ */ jsx4(Markdown2, { text: item.answer }),
    deliverables.length > 0 && /* @__PURE__ */ jsx4(Deliverables, { files: deliverables, sessionId }),
    item.streaming && item.answer && /* @__PURE__ */ jsx4("span", { className: "cc-caret" }),
    item.streaming && !item.answer && !hasProcess && /* @__PURE__ */ jsx4("div", { className: "cc-note", children: "\u2026" }),
    !item.streaming && (item.answer || usage && tokens > 0) && /* @__PURE__ */ jsxs3("div", { className: "cc-tail", "data-actions-reveal": "hover", children: [
      /* @__PURE__ */ jsx4("div", { className: "cc-actions", children: item.answer && /* @__PURE__ */ jsx4("button", { className: "cc-icon-btn", title: copied ? "\u5DF2\u590D\u5236" : "\u590D\u5236", onClick: () => copy(item.answer), children: /* @__PURE__ */ jsx4(Icon, { d: ICON.copy, size: 15 }) }) }),
      usage && tokens > 0 && /* @__PURE__ */ jsx4(UsagePill, { usage })
    ] })
  ] });
}
function UserBubble({
  text: text7,
  attachments,
  optimistic,
  id
}) {
  return /* @__PURE__ */ jsxs3("div", { className: "cc-user" + (optimistic ? " is-optimistic" : ""), id, children: [
    attachments && attachments.length > 0 && /* @__PURE__ */ jsx4("div", { className: "cc-user-images", children: attachments.map((a, i) => /* @__PURE__ */ jsx4(
      "img",
      {
        className: "cc-user-image",
        src: a.dataUrl,
        alt: a.name || "\u56FE\u7247",
        title: a.name || a.mimeType
      },
      i
    )) }),
    text7 && /* @__PURE__ */ jsx4("div", { className: "cc-user-text", children: text7 })
  ] });
}
function NoticeView({ text: text7, tone }) {
  return /* @__PURE__ */ jsx4("div", { className: "cc-notice" + (tone === "error" ? " is-error" : ""), children: text7 });
}
function CommandEcho({ name: name2, args }) {
  return /* @__PURE__ */ jsx4("div", { className: "cc-user cc-cmd-echo", children: /* @__PURE__ */ jsxs3("div", { className: "cc-user-text", children: [
    /* @__PURE__ */ jsxs3("span", { className: "cc-cmd-chip", children: [
      "/",
      name2
    ] }),
    args
  ] }) });
}
function CommandResultRow({ name: name2, outcome }) {
  const [open, setOpen] = useState4(false);
  const running = outcome === null;
  const text7 = outcome?.text ?? "";
  const failed = outcome?.kind === "error";
  const summary = running ? "\u6267\u884C\u4E2D\u2026" : text7 === "" ? failed ? "\u6307\u4EE4\u5931\u8D25" : "\u5DF2\u5B8C\u6210" : firstLine(text7, 90);
  const expandable = !running && text7.includes("\n");
  return /* @__PURE__ */ jsxs3("div", { className: "cc-cmd-result", "data-state": running ? "running" : failed ? "error" : "ok", children: [
    /* @__PURE__ */ jsxs3(
      "button",
      {
        className: "cc-row" + (expandable ? "" : " cc-row-static"),
        "aria-expanded": expandable ? open : void 0,
        onClick: () => expandable && setOpen((v) => !v),
        children: [
          /* @__PURE__ */ jsx4("span", { className: "cc-cmd-glyph", children: /* @__PURE__ */ jsx4(Icon, { d: ICON.terminal }) }),
          /* @__PURE__ */ jsx4("span", { className: "cc-tool-name", children: name2 || "\u6307\u4EE4" }),
          /* @__PURE__ */ jsx4("span", { className: "cc-tool-sep", "aria-hidden": "true" }),
          /* @__PURE__ */ jsx4("span", { className: "cc-row-summary" + (failed ? " err" : ""), children: summary })
        ]
      }
    ),
    open && expandable && /* @__PURE__ */ jsx4("div", { className: "cc-tool-details", children: /* @__PURE__ */ jsx4("pre", { className: failed ? "err" : void 0, children: text7 }) })
  ] });
}
function CommandView({ item }) {
  const rich = typeof item.sourceEventSeq === "number";
  return /* @__PURE__ */ jsxs3("div", { className: "cc-cmd", children: [
    /* @__PURE__ */ jsx4(CommandEcho, { name: item.name, args: item.args }),
    !rich && /* @__PURE__ */ jsx4(CommandResultRow, { name: item.name, outcome: item.outcome })
  ] });
}
function CompactionView({ item }) {
  const [open, setOpen] = useState4(false);
  const expandable = item.summary !== null;
  const summary = item.running ? "\u6B63\u5728\u538B\u7F29\u2026" : item.error ? `\u538B\u7F29\u5931\u8D25\uFF1A${item.error}` : item.shadowedItemCount !== null && item.shadowedTokenCount !== null ? `\u5DF2\u538B\u7F29 ${item.shadowedItemCount} \u6761\u5386\u53F2\u8BB0\u5F55\uFF08\u7EA6 ${formatTokens(item.shadowedTokenCount)} tokens\uFF09` : expandable ? "\u70B9\u51FB\u67E5\u770B\u538B\u7F29\u6458\u8981" : "\u538B\u7F29\u6458\u8981\u4E0D\u53EF\u7528";
  return /* @__PURE__ */ jsxs3("div", { className: "cc-compaction", children: [
    /* @__PURE__ */ jsxs3(
      "button",
      {
        className: "cc-compaction-btn",
        disabled: !expandable,
        "aria-expanded": expandable ? open : void 0,
        onClick: () => setOpen((v) => !v),
        children: [
          /* @__PURE__ */ jsxs3("span", { className: "cc-compaction-leading", "aria-hidden": "true", children: [
            /* @__PURE__ */ jsx4("span", { className: "cc-compaction-icon", children: /* @__PURE__ */ jsx4(Icon, { d: ICON.terminal }) }),
            /* @__PURE__ */ jsx4("span", { className: "cc-chevron" + (open ? " open" : ""), children: /* @__PURE__ */ jsx4(Icon, { d: ICON.chevron }) })
          ] }),
          /* @__PURE__ */ jsx4("span", { className: "cc-compaction-title", children: "\u4E0A\u4E0B\u6587\u5DF2\u538B\u7F29" }),
          /* @__PURE__ */ jsx4("span", { className: "cc-tool-sep", "aria-hidden": "true" }),
          /* @__PURE__ */ jsx4("span", { className: "cc-compaction-summary" + (item.error ? " err" : ""), children: summary })
        ]
      }
    ),
    open && item.summary !== null && /* @__PURE__ */ jsx4("div", { className: "cc-compaction-body", children: /* @__PURE__ */ jsx4(Markdown2, { text: item.summary }) })
  ] });
}
function ChatItems({ items, sessionId }) {
  return /* @__PURE__ */ jsx4(Fragment4, { children: items.map(
    (item, i) => item.kind === "user" ? /* @__PURE__ */ jsx4(
      UserBubble,
      {
        id: itemAnchor(i),
        text: item.text,
        attachments: item.attachments,
        optimistic: item.optimistic
      },
      i
    ) : item.kind === "notice" ? /* @__PURE__ */ jsx4(NoticeView, { text: item.text, tone: item.tone }, i) : item.kind === "command" ? /* @__PURE__ */ jsx4(CommandView, { item }, i) : item.kind === "compaction" ? /* @__PURE__ */ jsx4(CompactionView, { item }, i) : /* @__PURE__ */ jsx4(AssistantTurn, { id: itemAnchor(i), item, sessionId }, i)
  ) });
}
function Button({
  label,
  onClick,
  tone = "plain"
}) {
  return /* @__PURE__ */ jsx4(
    "button",
    {
      className: "cc-btn" + (tone === "primary" ? " is-primary" : tone === "danger" ? " is-danger" : ""),
      onClick,
      children: label
    }
  );
}
function safeJson(value) {
  try {
    return typeof value === "string" ? value : JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}
var SESSION_GRANT_DENIED = [
  "rm",
  "del",
  "erase",
  "rmdir",
  "rd",
  "format",
  "mkfs",
  "dd",
  "shutdown",
  "reboot",
  "sudo",
  "doas",
  "diskpart",
  "reg",
  "stop-process",
  "remove-item",
  "kill"
];
function sessionGrantLabel(input2) {
  if (!input2 || typeof input2 !== "object") return null;
  const command = input2.command;
  if (typeof command !== "string") return null;
  const normalized = command.trim().toLowerCase();
  if (!normalized) return null;
  if (["&&", "||", ";", "|", ">", "<", "`", "$("].some((m) => normalized.includes(m))) return null;
  const first = normalized.split(/\s+/)[0];
  return SESSION_GRANT_DENIED.includes(first) ? null : first;
}
function ApprovalCardView({
  card,
  onDecision
}) {
  const sandboxRetry = card.reason === "sandbox-retry";
  const grantLabel = sandboxRetry ? null : sessionGrantLabel(card.input);
  return /* @__PURE__ */ jsxs3("div", { className: "cc-card is-approval", children: [
    /* @__PURE__ */ jsx4("div", { className: "cc-card-strip", children: sandboxRetry ? "\u6C99\u7BB1\u62D2\u7EDD\u540E\u7533\u8BF7\u8D8A\u754C\u6267\u884C" : "\u9700\u8981\u4F60\u7684\u786E\u8BA4" }),
    /* @__PURE__ */ jsxs3("div", { className: "cc-card-body", children: [
      /* @__PURE__ */ jsxs3("div", { className: "cc-card-title", children: [
        "\u5DE5\u5177\u5BA1\u6279\uFF1A",
        card.tool
      ] }),
      /* @__PURE__ */ jsx4("div", { className: "cc-card-meta", children: truncate(safeJson(card.input), 600) })
    ] }),
    /* @__PURE__ */ jsxs3("div", { className: "cc-card-actions", children: [
      /* @__PURE__ */ jsx4(Button, { label: "\u62D2\u7EDD", tone: "danger", onClick: () => onDecision(card.requestId, "deny") }),
      /* @__PURE__ */ jsx4(Button, { label: "\u6279\u51C6", tone: "primary", onClick: () => onDecision(card.requestId, "approve") }),
      grantLabel ? /* @__PURE__ */ jsx4(
        Button,
        {
          label: `\u672C\u4F1A\u8BDD\u5141\u8BB8 ${card.tool}: ${grantLabel}`,
          onClick: () => onDecision(card.requestId, "approve_for_session")
        }
      ) : null
    ] })
  ] });
}
function PlanReviewCardView({
  card,
  onVerdict
}) {
  return /* @__PURE__ */ jsxs3("div", { className: "cc-card", children: [
    /* @__PURE__ */ jsx4("div", { className: "cc-card-strip", children: "\u8BA1\u5212\u5F85\u786E\u8BA4" }),
    /* @__PURE__ */ jsx4("div", { className: "cc-card-body", children: /* @__PURE__ */ jsx4(Markdown2, { text: card.plan ?? "" }) }),
    /* @__PURE__ */ jsxs3("div", { className: "cc-card-actions", children: [
      /* @__PURE__ */ jsx4(Button, { label: "\u653E\u5F03", onClick: () => onVerdict(card.requestId, "dismissed") }),
      /* @__PURE__ */ jsx4(Button, { label: "\u7EE7\u7EED\u89C4\u5212", onClick: () => onVerdict(card.requestId, "keep_planning") }),
      /* @__PURE__ */ jsx4(Button, { label: "\u6279\u51C6\u6267\u884C", tone: "primary", onClick: () => onVerdict(card.requestId, "approve") })
    ] })
  ] });
}
function QuestionCardView({
  card,
  onSubmit
}) {
  const questions = Array.isArray(card.questions) ? card.questions : [];
  const [selected, setSelected] = useState4({});
  const [custom, setCustom] = useState4({});
  const toggle = (qid, label, multi) => {
    setSelected((prev) => {
      const current = prev[qid] ?? [];
      if (!multi) return { ...prev, [qid]: current.includes(label) ? [] : [label] };
      return {
        ...prev,
        [qid]: current.includes(label) ? current.filter((l) => l !== label) : [...current, label]
      };
    });
  };
  const submit = () => {
    const answers = questions.map((q) => ({
      id: q.id,
      selected: selected[q.id] ?? [],
      custom: custom[q.id]?.trim() || void 0
    }));
    onSubmit(card.requestId, answers);
  };
  return /* @__PURE__ */ jsxs3("div", { className: "cc-card is-question", children: [
    questions.map((q) => /* @__PURE__ */ jsxs3("div", { className: "cc-question", children: [
      q.header && /* @__PURE__ */ jsx4("div", { className: "cc-question-head", children: q.header }),
      /* @__PURE__ */ jsx4("div", { className: "cc-question-text", children: q.question }),
      (q.options ?? []).map((opt, idx) => {
        const on = (selected[q.id] ?? []).includes(opt.label);
        return /* @__PURE__ */ jsxs3(
          "button",
          {
            className: "cc-option" + (on ? " on" : ""),
            onClick: () => toggle(q.id, opt.label, q.multiSelect === true),
            children: [
              /* @__PURE__ */ jsx4("span", { className: "cc-option-mark", children: on ? "\u2713" : idx + 1 }),
              /* @__PURE__ */ jsxs3("span", { children: [
                opt.label,
                opt.description && /* @__PURE__ */ jsxs3("span", { className: "cc-option-desc", children: [
                  " \u2014 ",
                  opt.description
                ] })
              ] })
            ]
          },
          opt.label
        );
      }),
      /* @__PURE__ */ jsx4(
        "input",
        {
          className: "cc-question-custom",
          placeholder: "\u81EA\u5B9A\u4E49\u56DE\u7B54\uFF08\u53EF\u9009\uFF09",
          value: custom[q.id] ?? "",
          onChange: (e) => setCustom((prev) => ({ ...prev, [q.id]: e.target.value }))
        }
      )
    ] }, q.id)),
    /* @__PURE__ */ jsx4("div", { className: "cc-card-actions", style: { padding: "var(--ava-space-2, 8px) 0 0" }, children: /* @__PURE__ */ jsx4(Button, { label: "\u63D0\u4EA4", tone: "primary", onClick: submit }) })
  ] });
}

// src/styles.ts
var chatappCss = `
.cc-root,
.cc-pop {
  /* \u5BBF\u4E3B design token \u5951\u7EA6\uFF08src/styles/tokens.css \u7684 --ava-*\uFF09\uFF1AApp \u4E0E AppHost \u5171\u7528\u4E00\u5957\u8BED\u4E49\u4EE4\u724C\uFF0C
     \u4EAE/\u6697\u968F documentElement \u7684 data-theme \u7EE7\u627F\uFF1B\u5BBF\u4E3B\u7F3A\u5931\u65F6\u56DE\u9000\u5230\u5404 var() \u5185\u7F6E\u7684\u4EAE\u8272\u503C\u3002 */
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
  /* App \u4E1A\u52A1\u4E13\u6709\u8272\u5F69\uFF08\u4E0D\u8FDB\u5BBF\u4E3B\u5951\u7EA6\uFF09 */
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
  /* \u4EC5\u8986\u76D6 App \u4E13\u6709\u8272\u5F69\uFF1B\u8BED\u4E49\u4EE4\u724C\u7531\u5BBF\u4E3B --ava-* \u968F\u4E3B\u9898\u81EA\u52A8\u5207\u6362 */
  --cc-bubble: #2c2c2e;
  --cc-code-bg: #1b1b1c;
  --cc-inline-code: rgba(255, 255, 255, 0.08);
  --cc-input: #2c2c2e;
  --cc-warn-bg: rgba(240, 180, 41, 0.14);
  --cc-warn-line: rgba(240, 180, 41, 0.45);
}

/* \u2500\u2500 \u4F1A\u8BDD\u5934\uFF1A\u6700\u5C0F\u9AD8 76px\uFF0C\u5DE6\u53F3\u5185\u8FB9\u8DDD 20/28 \u2500\u2500 */
.cc-head {
  display: flex;
  align-items: center;
  gap: var(--ava-space-4, 16px);
  min-height: 52px;
  padding: var(--ava-space-3, 10px) var(--ava-space-5, 24px) var(--ava-space-3, 10px) var(--ava-space-5, 24px);
  border-bottom: 0.5px solid var(--cc-l3);
  flex: none;
}

/* \u4E2D\u680F\u89C6\u56FE\u5206\u6BB5 tab\uFF08\u5BF9\u8BDD / \u8F68\u8FF9\uFF09\uFF1A\u5206\u6BB5\u63A7\u4EF6\u5F0F\uFF0C\u6FC0\u6D3B\u5E95 + \u4E3B\u6587\u672C\u8272 */
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

/* \u5934\u90E8\u53F3\u7AEF\uFF1A\u751F\u6210\u72B6\u6001 + \u4E0A\u4E0B\u6587\u5360\u6BD4 */
.cc-head-right {
  display: flex;
  align-items: center;
  gap: var(--ava-space-3, 10px);
  margin-left: auto;
  min-width: 0;
}

/* \u4E0A\u4E0B\u6587\u5360\u6BD4\uFF1A4px \u8FDB\u5EA6\u6761 + \u4F30\u7B97\u6570\u503C\uFF08\u226570% \u8B66\u544A\u3001\u226590% \u5371\u9669\uFF09 */
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

/* \u2500\u2500 \u5DE5\u4F5C\u76EE\u5F55\u9009\u62E9\u5668\uFF08\u6A21\u6001\uFF09\uFF1A\u4EE5\u300C\u6D4F\u89C8 + \u9009\u62E9\u300D\u4EE3\u66FF\u624B\u8F93\u8DEF\u5F84 \u2500\u2500 */
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

/* \u6700\u8FD1\u4F7F\u7528\u7684\u5DE5\u4F5C\u533A\uFF1A\u76EE\u5F55\u9009\u62E9\u5668\u5185\u7684\u5FEB\u6377\u5165\u53E3 */
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

/* \u300C\u6D4F\u89C8\u7CFB\u7EDF\u76EE\u5F55\u2026\u300D\uFF1A\u5DE6\u5BF9\u9F50\u7684\u6B21\u7EA7\u5165\u53E3\uFF08\u5524\u8D77 OS \u539F\u751F\u6587\u4EF6\u5939\u9009\u62E9\u5668\uFF09 */
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

/* \u5360\u4F4D\uFF1A\u628A\u53D6\u6D88/\u4F7F\u7528\u6B64\u76EE\u5F55\u63A8\u5230\u53F3\u4FA7\uFF0C\u6D4F\u89C8\u7CFB\u7EDF\u76EE\u5F55\u7559\u5728\u5DE6\u4FA7 */
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

/* \u2500\u2500 \u6D88\u606F\u6D41\uFF08\u5217\u5BBD clamp(680,64%,920)\uFF0C\u4E24\u4FA7 32px\uFF09 \u2500\u2500 */
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

/* \u7A7A\u6001\u56FE\u6807\u4F4D\uFF1A44px \u5706\u89D2\u5E95\u5EA7\uFF0C\u5BF9\u9F50\u5BBF\u4E3B\u7A7A\u6001\u7EC4\u4EF6\u89C2\u611F */
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

/* \u7528\u6237\u6D88\u606F\uFF1A\u53F3\u5BF9\u9F50\u6C14\u6CE1\uFF08radius 20 / padding 10-16 / \u884C\u9AD8 22\uFF09 */
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

/* \u7528\u6237\u6C14\u6CE1\u5185\u7684\u56FE\u7247\u9644\u4EF6\uFF1A\u7F29\u7565\u56FE\u5E76\u6392\uFF0C\u5706\u89D2\u4E0E\u6C14\u6CE1\u4E00\u81F4 */
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

/* \u52A9\u624B\u6D88\u606F\uFF1A\u65E0\u6C14\u6CE1\u65E0\u8FB9\u8DDD\u8FB9\u6846\uFF0C\u4EC5 markdown \u6B63\u6587 */
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

/* \u2500\u2500 \u8F6E\u6B21\u8FC7\u7A0B\u533A\uFF1A33px \u5934 + \u534A\u50CF\u7D20\u4E0B\u8FB9\u7EBF\uFF0C\u9ED8\u8BA4\u6298\u53E0 \u2500\u2500 */
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

/* \u2500\u2500 \u601D\u8003\u884C\uFF1A24px \u884C\u9AD8\uFF0Ctertiary \u2192 hover secondary \u2500\u2500 */
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

/* \u6D41\u5F0F\u6458\u8981\u5FAE\u5149\uFF08dsh TextShimmer \u5BF9\u9F50\uFF09\uFF1A\u6587\u5B57\u6E10\u53D8\u5F80\u590D\u626B\u8FC7 */
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

/* todo_write \u7684\u5E76\u884C\u8FDB\u884C\u9879\u4F59\u6570\uFF1A+N \u540E\u7F00\uFF0C\u4E0D\u53C2\u4E0E\u7701\u7565 */
.cc-tool-extra {
  flex: none;
  color: var(--cc-t3);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}

/* \u601D\u8003\u5C55\u5F00\u6B63\u6587\uFF1Amarkdown \u6B21\u7EA7\u6392\u7248\uFF08dsh \u5BF9\u9F50\u2014\u2014\u4E0D\u518D\u7528 pre-wrap\uFF0C\u7531 cc-md \u63A5\u7BA1\uFF09 */
.cc-thinking {
  margin: var(--ava-space-0, 2px) 0 var(--ava-space-2, 8px);
  padding-left: var(--ava-space-5, 24px);
  color: var(--cc-t3);
  font-size: 13px;
  line-height: 20px;
  word-break: break-word;
}

/* \u2500\u2500 \u5DE5\u5177\u884C \u2500\u2500 */
.cc-tool {
  display: flex;
  flex-direction: column;
}

.cc-tool-name {
  color: var(--cc-t2);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
}

/* \u6298\u53E0\u5934\u7684\u5206\u9694\u70B9\uFF1A2\xD72px \u5706\u70B9\uFF08label caption\uFF09\uFF0C\u9760 flex gap \u7559\u767D */
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

/* \u5DE5\u5177\u8BE6\u60C5\uFF1A\u534A\u50CF\u7D20 l1 + radius 16 + \u4EE3\u7801\u5E95\u8272 */
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

/* \u2500\u2500 markdown\uFF1Ah2 19/28\u3001p 16px 0\u3001\u5217\u8868 18px \u7F29\u8FDB \u2500\u2500 */
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
  /* markdown\uFF1A\u6807\u9898\u4E0A 32px / \u4E0B 16px */
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
  /* markdown\uFF1A\u5217\u8868\u7F29\u8FDB 18px\uFF0C\u6761\u76EE\u95F4\u8DDD 6px */
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

/* markdown \u8868\u683C\uFF1A\u4EC5\u4E0B\u8FB9\u7EBF\u3001\u5355\u5143\u683C 10/16\u3001\u9996\u5217\u4E0D\u7559\u5DE6\u5185\u8FB9\u8DDD */
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

/* \u4EE3\u7801\u5757\uFF1A\u4EE3\u7801\u5E95\u8272 + radius 16 + sticky \u8BED\u8A00\u6761 */
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
  /* \u4EE3\u7801\u5757\uFF1A\u4E0D\u6A2A\u5411\u6EDA\u52A8\uFF0C\u957F\u884C\u8F6F\u6362\u884C */
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

/* \u2500\u2500 \u8F6E\u6B21\u672B\u5C3E\uFF1A\u590D\u5236\u64CD\u4F5C + \u672C\u8F6E token \u7528\u91CF \u2500\u2500
   \u663E\u9690\u7531 data-actions-reveal \u63A7\u5236\uFF08items.tsx \u7ED9\u51FA\uFF09\u2014\u2014
   \u6700\u540E\u4E00\u8F6E\u300Calways\u300D\u5E38\u663E\uFF1B\u5386\u53F2\u8F6E\u6B21\u300Chover\u300D\u4EC5\u60AC\u505C/\u805A\u7126\u65F6\u663E\u793A\uFF08opacity \u4FDD\u6301\u5E03\u5C40\u7A33\u5B9A\uFF0C
   80ms \u8FC7\u6E21\uFF1B\u65E0 hover \u6307\u9488\u8BBE\u5907\u4E00\u5F8B\u5E38\u663E\uFF1B\u7528\u91CF\u5F39\u5C42\u6253\u5F00\u65F6\u89E6\u53D1\u5668\u4FDD\u6301\u53EF\u89C1\uFF09\u3002 */
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

/* \u8F6E\u672B\u7528\u91CF pill\uFF1A15px \u6570\u636E\u5E93\u56FE\u6807 + \u300C\u7528\u91CF N tok\u300D\uFF0C28px \u9AD8 /
   6\xD78 \u5185\u8FB9\u8DDD / 4px \u56FE\u6807\u95F4\u9699\uFF0Ctertiary \u8272 + tabular \u6570\u503C\uFF0Chover \u4E0E\u5C55\u5F00\u65F6\u663E\u4EA4\u4E92\u5E95\u8272 */
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

/* \u7A84\u5217\u65F6\u6807\u7B7E\u622A\u65AD\u4E3A\u7701\u7565\u53F7\uFF0C\u6570\u636E\u5E93\u56FE\u6807\u4E0D\u6536\u7F29 */
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

/* \u672C\u8F6E\u7528\u91CF\u660E\u7EC6\u5F39\u5C42\uFF1A\u6807\u9898\u884C\u5DE6\u540D\u53F3\u7CBE\u786E\u603B\u91CF\u30010.5px \u5206\u9694\u7EBF\u3001
   dl \u53CC\u5217 76px/auto + 1fr\uFF0Cdt tertiary / dd secondary \u53F3\u5BF9\u9F50 tabular */
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

/* \u2500\u2500 \u6D88\u606F\u64CD\u4F5C\u884C\uFF1A28px / gap 8 / 15px \u56FE\u6807 / hover \u663E\u793A \u2500\u2500 */
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

/* \u2500\u2500 \u63D0\u793A\u884C\uFF08\u6A21\u578B\u91CD\u8BD5 / \u9519\u8BEF\uFF09 \u2500\u2500 */
.cc-note {
  color: var(--cc-warn);
  font-size: 12px;
  line-height: 18px;
}

/* \u2500\u2500 \u5F85\u56DE\u586B\u5361\u7247 \u2500\u2500 */
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

/* \u2500\u2500 composer\uFF1A\u5217\u5BBD+32\u3001radius 28\u3001\u534A\u50CF\u7D20\u63CF\u8FB9 + \u8F6F\u9634\u5F71 \u2500\u2500 */
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

/* \u2500\u2500 \u8F93\u5165\u5361\u7247\u5E95\u90E8\u5DE5\u5177\u884C\uFF1A\u5DE6\u7AEF\u5DE5\u4F5C\u533A/\u5BA1\u6279\uFF0C\u53F3\u7AEF\u6A21\u578B/\u53D1\u9001 \u2500\u2500 */
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

/* chip \u89E6\u53D1\u5668\uFF1A28px \u9AD8\u3001radius-sm\u3001
   \u56FE\u6807 + \u6587\u5B57 + \u53F3\u4FA7\u5C55\u5F00\u7BAD\u5934\uFF0C\u70B9\u51FB\u5C55\u5F00\u951A\u5B9A\u4E0A\u6D6E\u9762\u677F\u3002 */
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

/* \u9644\u4EF6\u6309\u94AE\uFF08composer \u5DE5\u5177\u6761\u6700\u5DE6\uFF09\uFF1A\u4E0E chip \u540C\u9AD8 28px \u7684\u7EAF\u56FE\u6807\u6309\u94AE */
.cc-attach-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  /* \u5168\u5C40 button reset \u5E26 padding: 4px 10px\uFF0C\u4F1A\u8BA9 28px \u6309\u94AE\u7684 content \u533A\u53EA\u5269 8px\uFF0C
     \u56FE\u6807\u88AB flex \u538B\u6241\uFF1B\u7EAF\u56FE\u6807\u6309\u94AE\u5FC5\u987B\u6E05\u96F6\u5185\u8FB9\u8DDD\u3002 */
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

/* \u5F85\u53D1\u9001\u9644\u4EF6\u9884\u89C8\u6761\uFF08composer \u5185\u3001\u8F93\u5165\u6846\u4E0A\u65B9\uFF09 */
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

/* \u2500\u2500 \u4E0A\u6D6E\u9762\u677F\uFF08\u951A\u5B9A composer chip\uFF1Bportal \u5230 body\uFF0C\u6545\u6837\u5F0F\u4E0D\u4F9D\u8D56 .cc-root \u7ED3\u6784\uFF09 \u2500\u2500 */
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

/* \u751F\u6210\u4E2D\uFF1A\u53D1\u9001\u952E\u5207\u6210\u505C\u6B62\u952E\uFF08\u6DF1\u8272\u5B9E\u5FC3\u5E95 + \u65B9\u5F62\u56FE\u6807\uFF0C\u4E0E\u53EF\u53D1\u9001\u6001\u660E\u663E\u533A\u5206\uFF09 */
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

/* \u2500\u2500 \u4F1A\u8BDD\u8F68\u8FF9\u89C6\u56FE\uFF08\u72EC\u7ACB tab\uFF1A\u5DE5\u5177\u680F + \u6CF3\u9053\u6982\u89C8 + \u56DE\u5408\u5206\u7EC4\u8BB0\u5F55\u8868\uFF09 \u2500\u2500
   \u8272\u6807\u8BED\u4E49\uFF08\u5BF9\u9F50 ava_square\uFF09\uFF1A\u7528\u6237=\u4E2D\u7070\u3001\u6D88\u606F=\u84DD\u3001\u601D\u8003=\u6D45\u84DD\u3001\u5DE5\u5177=\u9752\u3001
   \u4E0A\u4E0B\u6587/\u7CFB\u7EDF=\u4E2D\u6027\u7070\u3001\u9519\u8BEF=\u7EA2\u3002 */
.cc-trj-root {
  /* \u672C\u5730\u522B\u540D \u2192 \u5BBF\u4E3B\u4EE4\u724C\u6D3E\u751F\uFF08color-mix \u968F\u4EAE/\u6697\u4E3B\u9898\u81EA\u52A8\u5207\u6362\uFF09 */
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

/* \u5DE5\u5177\u680F\uFF1A\u641C\u7D22 + \u8BB0\u5F55\u6570 + \u6298\u53E0\u5168\u90E8 */
.cc-trj-toolbar {
  display: flex;
  align-items: center;
  gap: var(--ava-space-2, 8px);
  flex: none;
  height: 40px;
  padding: 0 var(--ava-space-4, 16px);
  border-bottom: 0.5px solid var(--cc-l3);
}

/* \u641C\u7D22\u6846\uFF1A\u56FE\u6807\u5185\u5D4C\uFF08ava_square Search \u56FE\u6807\u540C\u4F4D\uFF09 */
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

/* \u4E09\u6CF3\u9053\u6982\u89C8\uFF08\u5E8F\u5217\u6A21\u5F0F\uFF09\uFF1A\u7EDF\u8BA1\u884C + \u5DE6\u6807\u7B7E\u5217 + \u53F3\u8F68\u9053\u5217\uFF1B
   \u5757\u4F4D\u7F6E = (index-1)/span \u7B49\u5BBD\u6392\u5E03\uFF0C\u4E0D\u8868\u8FBE\u65F6\u957F\uFF08\xA79.3\uFF09\u3002 */
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

/* \u8272\u6807\u8BED\u4E49\uFF08\u5BF9\u9F50 ava_square\uFF09\uFF1A\u7528\u6237=\u4E2D\u7070\u3001\u6D88\u606F=\u84DD\u3001\u601D\u8003=\u6D45\u84DD\u3001
   \u5DE5\u5177=\u9752\uFF08blue\xD7success \u6DF7\u8272\uFF09\u3001\u4E0A\u4E0B\u6587=\u6D45\u7070\u3001\u7CFB\u7EDF=\u7070\u3001\u9519\u8BEF=\u7EA2 */
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

/* \u56DE\u5408\u5206\u7EC4\u8BB0\u5F55\u8868\uFF1A\u7C98\u6027\u8F6E\u5934 + \u8BB0\u5F55\u884C */
.cc-trj-table {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.cc-trj-turn {
  display: flex;
  flex-direction: column;
}

/* \u7C98\u6027\u8F6E\u5934\uFF1A32px\uFF08h-8\uFF09\uFF1B\u8FDB\u884C\u4E2D\u5DE6\u7F18 2px accent \u6761 */
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

/* \u8BB0\u5F55\u884C\uFF1A28px\uFF0Cgrid [52px \u6807\u7B7E | 1fr \u6458\u8981 | auto \u6307\u6807 | 14px \u7BAD\u5934]\uFF08\xA79.4\uFF09 */
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

/* \u7C7B\u522B\u8272\u6807 chip\uFF1A10px \u7B49\u5BBD\u77ED\u7801\uFF08U/MSG/THINK/TOOL/SYS/CTX/ERR\uFF09\uFF0C\u6309 kind \u4E0A\u8272 */
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

/* \u4E3B/\u6B21\u6458\u8981\uFF1A\u6709\u6B21\u7EA7\u6458\u8981\u65F6\u6807\u9898\u53D6 45% \u5BBD\u5E76\u8F6C\u7B49\u5BBD\uFF08ava_square \u540C\u6B3E\uFF09 */
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

/* \u5C55\u5F00\u8BE6\u60C5\uFF1A\u7B49\u5BBD pre\uFF0C\u6700\u9AD8 256px \u5185\u6EDA\uFF08tool \u884C\u663E\u793A input/output \u539F\u6587\uFF09 */
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

/* \u2500\u2500 \u4FA7\u8FB9\u680F\u4F1A\u8BDD\u5386\u53F2\uFF08panel.sidebar\uFF09 \u2500\u2500 */
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
  /* \u5217\u8868\u5DE6\u4FA7 4px \u5185\u7F29\uFF08\u76F8\u5BF9\u4FA7\u680F 12px \u5185\u8FB9\u8DDD\uFF09\uFF0C\u53F3\u4FA7\u7559 2px \u7ED9\u6EDA\u52A8\u6761 */
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

/* Popconfirm \u6C14\u6CE1\u786E\u8BA4\uFF08\u6D6E\u5C42\u6307\u5357\uFF1A\u951A\u5B9A\u89E6\u53D1\u70B9\u3001\u70B9\u5916\u53D6\u6D88\u3001\u65E0\u906E\u7F69\uFF09 */
.cc-pc-anchor {
  position: relative;
}

.cc-pc-anchor.head {
  position: absolute;
  right: 0;
}

.cc-pc {
  /* fixed + JS \u6309\u951A\u70B9 rect \u8BA1\u7B97\uFF08\u89C1 session-sidebar.tsx\uFF09\uFF1A\u907F\u5F00\u6EDA\u52A8\u5BB9\u5668 overflow \u88C1\u526A */
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

/* \u72B6\u6001\u70B9\u69FD\u4F4D\uFF1A16px \u56FA\u5B9A\u69FD\uFF0C\u65E0 live \u65F6\u7559\u7A7A\u4EE5\u7EF4\u6301\u6807\u9898\u5BF9\u9F50 */
.cc-sb-row-slot {
  display: inline-grid;
  place-items: center;
  width: 16px;
  height: 16px;
  flex: none;
}

/* \u8FD0\u884C\u4E2D\u4F1A\u8BDD\u72B6\u6001\u70B9\uFF1A10px \u5706\u70B9 */
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

/* \u4F1A\u8BDD\u884C\u5BB9\u5668\uFF1Ahover \u65F6\u4EE5\u64CD\u4F5C\u7EC4\u66FF\u6362\u76F8\u5BF9\u65F6\u95F4 */
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

/* present \u4EA4\u4ED8\u7269\u5361\u7247\uFF08\u5DE5\u5177\u884C\u4E0B\u65B9\uFF09 */
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

/* \u2500\u2500 \u659C\u6760\u547D\u4EE4\u56DE\u6267\uFF08\u672C\u5730\u6267\u884C\u7ED3\u679C\uFF0C\u4E0D\u8FDB\u8F6E\u6B21\u5757\uFF1Berror \u6001\u8F6C\u7EA2\uFF09 \u2500\u2500 */
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

/* \u2500\u2500 \u547D\u4EE4\u6761\u76EE\uFF1A\u53F3\u5BF9\u9F50\u56DE\u663E\u6C14\u6CE1 + \u7ED3\u679C\u884C \u2500\u2500 */
.cc-cmd {
  display: flex;
  flex-direction: column;
  gap: var(--ava-space-1, 4px);
}

/* \u56DE\u663E\u6C14\u6CE1\u91CC\u7684\u547D\u4EE4 chip\uFF08mono + \u6D45\u5E95\uFF09 */
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

/* \u2500\u2500 \u4E0A\u4E0B\u6587\u538B\u7F29\u6807\u8BB0\uFF1A\u56FE\u6807 + \u6807\u9898 + \u8BA1\u6570\u6458\u8981\uFF0C\u53EF\u5C55\u5F00 \u2500\u2500 */
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

/* \u2500\u2500 \u5F85\u529E\u9762\u677F\uFF08composer \u4E0A\u65B9 dock \u5361\u7247\uFF0C\u7A7A\u6E05\u5355\u4E0D\u5360\u4F4D\u3001\u9ED8\u8BA4\u6298\u53E0\uFF09 \u2500\u2500 */
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

/* \u4E0E GoalBar \u6807\u9898\u540C\u89C4\u683C\uFF0813/24 medium primary\uFF09\uFF0C\u4E09\u5F20 dock \u5361\u7247\u8BFB\u4F5C\u540C\u4E00\u65CF */
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

/* \u2500\u2500 \u5F85\u529E\u72B6\u6001\u70B9\uFF1A\u5B8C\u6210 = 6px \u5B9E\u5FC3\u70B9 / \u8FDB\u884C\u4E2D = 14px \u65CB\u8F6C\u73AF / \u5F85\u5904\u7406 = \u4E2D\u6027\u70B9 \u2500\u2500 */
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

/* \u73AF\u5468\u957F 2\u03C0r \u2248 37.7\uFF0C\u5F27\u5360 1/4 */
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

/* \u2500\u2500 \u4F1A\u8BDD\u76EE\u6807\u6A2A\u5E45\uFF08composer \u4E0A\u65B9 dock\uFF0Corder 10\uFF1B36px \u5361\u7247 + \u5706\u5F62\u56FE\u6807\u52A8\u4F5C\uFF09 \u2500\u2500 */
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

/* phase \u6807\u7B7E\u5E26\u300C\u76EE\u6807\u300D\u540E\u7F00\uFF08\u5982 active \u2192 \u8FDB\u884C\u4E2D\u7684\u76EE\u6807\uFF09 */
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

/* \u6761\u5185\u5185\u8054\u7F16\u8F91\u8F93\u5165\uFF08\u5360\u6EE1\u76EE\u6807\u6587\u672C\u4F4D\uFF0C\u65E0\u8FB9\u6846\uFF09 */
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

/* \u2500\u2500 plan \u6A21\u5F0F\u5FBD\u6807\uFF08composer \u5E95\u90E8\u5DE5\u5177\u6761\uFF1B\u70B9\u51FB\u5373\u9000\u51FA\uFF0Chover \u6362\u5173\u56FE\u6807\uFF09 \u2500\u2500 */
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

/* \u2500\u2500 \u659C\u6760\u547D\u4EE4\u83DC\u5355\uFF1A
     combobox \u6A21\u5F0F\u2014\u2014\u7126\u70B9\u7559\u5728 textarea\uFF0Clistbox \u6302\u5728\u6EDA\u52A8\u89C6\u53E3\uFF1B
     \u8868\u9762 = \u5BBF\u4E3B\u6D6E\u5C42\u5951\u7EA6\uFF08layer-1 \u5E95 + 0.5px \u53D1\u4E1D\u63CF\u8FB9 + Panel \u91CD\u6D6E\u8D77\uFF0Clg \u5706\u89D2\uFF0Cz 30\uFF09\uFF1B
     \u884C\u9AD8 \u226534\u3001\u5171\u4EAB\u9AD8\u4EAE\u3001\u56FE\u6807 14/label-primary-dimmed\u3001\u63CF\u8FF0\u53F3\u5BF9\u9F50\uFF0C\u6EA2\u51FA\u5E95\u90E8\u6E10\u53D8\u63D0\u793A\u3002 \u2500\u2500 */
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

/* \u6EA2\u51FA\u4E0B\u65B9\u65F6\u5E95\u90E8\u6E10\u53D8\u63D0\u793A\uFF08data-overflow-below\uFF09\uFF0C\u6EDA\u5230\u5E95\u7531 JS \u79FB\u9664\u7C7B */
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

/* \u5355\u4E00\u5171\u4EAB\u9AD8\u4EAE\uFF1A\u6307\u9488\u79FB\u52A8\u4E0E\u952E\u76D8\u5171\u7528 .on\uFF0C\u65E0\u72EC\u7ACB :hover \u8272\u9636\u7ADE\u4E89 */
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

/* \u672C\u5730\u5316\u6807\u7B7E\u65C1\u7684\u547D\u4EE4\u540D\u522B\u540D\uFF1A\u6B21\u8981\u58A8\u8272\u3001\u968F\u957F\u7701\u7565 */
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

/* \u63CF\u8FF0\u53F3\u5BF9\u9F50\uFF1A\u6BCF\u884C\u63CF\u8FF0\u6536\u5728\u540C\u4E00\u6761\u53F3\u8FB9\u7F18\uFF0C\u6807\u9898\u8BFB\u6210\u4E00\u5217 */
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

// src/trajectory.tsx
import { useEffect as useEffect4, useMemo, useRef as useRef3, useState as useState5 } from "react";

// src/trajectory-model.ts
var TAG_BY_KIND = {
  user: "U",
  message: "MSG",
  thinking: "THINK",
  tool: "TOOL",
  system: "SYS",
  context: "CTX",
  error: "ERR"
};
var oneLine = (text7, max = 160) => firstLine(text7, max);
function splitSummary(text7, shortMax, longMax = 160) {
  const title = oneLine(text7, shortMax);
  const summary = firstLine(text7, longMax);
  return { title, summary: summary !== title ? summary : void 0 };
}
function toolDetail(inputText, resultText) {
  return [
    inputText.trim() ? `input:
${inputText.trim()}` : "",
    resultText?.trim() ? `output:
${resultText.trim()}` : ""
  ].filter(Boolean).join("\n\n");
}
function assignTurns(items) {
  const turnOrdinal = /* @__PURE__ */ new Map();
  let counter = 0;
  for (let i = 0; i < items.length; i++) {
    if (items[i].kind === "turn") turnOrdinal.set(i, ++counter);
  }
  const nextTurnOrdinal = (from) => {
    for (const [idx, ord] of turnOrdinal) if (idx > from) return ord;
    return 0;
  };
  const ordinals = new Array(items.length).fill(0);
  let current = 0;
  for (let i = 0; i < items.length; i++) {
    const known = turnOrdinal.get(i);
    if (known !== void 0) {
      current = known;
      ordinals[i] = known;
      continue;
    }
    const item = items[i];
    ordinals[i] = item.kind === "user" ? nextTurnOrdinal(i) || counter + 1 : current || nextTurnOrdinal(i);
  }
  return ordinals;
}
function cellsFromTurn(item, turn) {
  const cells = [];
  const usage = turnUsageOf(item);
  const finalize = (cell) => {
    const full = {
      id: "",
      index: 0,
      ...cell,
      searchText: [cell.tag, cell.title, cell.summary ?? "", cell.detail ?? ""].join("\n").toLowerCase()
    };
    return full;
  };
  let firstThinking = true;
  const thinkingCell = (seg) => {
    if (seg.trim() === "") return;
    const { title, summary } = splitSummary(seg, 80);
    cells.push(
      finalize({
        turn,
        kind: "thinking",
        tag: TAG_BY_KIND.thinking,
        title,
        summary,
        detail: seg,
        // 首段思考行携带该回合输出 token（表格行 token 展示口径不变）
        tokens: usage && firstThinking ? usage.outputTokens : void 0
      })
    );
    firstThinking = false;
  };
  for (let i = 0; i < item.steps.length; i++) {
    thinkingCell(item.thinkingSegs[i] ?? "");
    const step = item.steps[i];
    cells.push(
      finalize({
        turn,
        kind: "tool",
        tag: TAG_BY_KIND.tool,
        title: step.name,
        summary: step.inputText.trim() ? firstLine(step.inputText, 100) : void 0,
        detail: toolDetail(step.inputText, step.resultText),
        ok: step.running ? void 0 : !step.isError,
        running: step.running
      })
    );
  }
  thinkingCell(item.thinkingSegs[item.steps.length] ?? "");
  if (item.answer.trim() !== "") {
    const { title, summary } = splitSummary(item.answer, 80);
    cells.push(
      finalize({
        turn,
        kind: "message",
        tag: TAG_BY_KIND.message,
        title,
        summary,
        detail: item.answer,
        tokens: usage?.outputTokens
      })
    );
  }
  if (item.retryText) {
    cells.push(
      finalize({
        turn,
        kind: "error",
        tag: TAG_BY_KIND.error,
        title: item.streaming ? "\u8BF7\u6C42\u91CD\u8BD5\u4E2D" : "\u8BF7\u6C42\u5931\u8D25\u91CD\u8BD5",
        summary: firstLine(item.retryText, 100),
        detail: item.retryText
      })
    );
  }
  return cells;
}
function attachmentNote(count) {
  return `[\u56FE\u7247 \xD7${count}]`;
}
function buildTrajectory(items) {
  const ordinals = assignTurns(items);
  const cells = [];
  const turnMap = /* @__PURE__ */ new Map();
  const turns = [];
  const ensureTurn = (turn) => {
    let group = turnMap.get(turn);
    if (!group) {
      group = {
        turn,
        reason: "",
        steps: 0,
        streaming: false,
        cells: [],
        tools: 0,
        input: 0,
        output: 0,
        failed: 0
      };
      turnMap.set(turn, group);
      turns.push(group);
    }
    return group;
  };
  items.forEach((item, i) => {
    const turn = ordinals[i];
    const finalize = (cell) => ({
      id: "",
      index: 0,
      ...cell,
      searchText: [cell.tag, cell.title, cell.summary ?? "", cell.detail ?? ""].join("\n").toLowerCase()
    });
    switch (item.kind) {
      case "user": {
        const text7 = item.text.trim();
        const note = item.attachments?.length ? attachmentNote(item.attachments.length) : "";
        const { title, summary } = splitSummary(text7 || note, 80);
        cells.push(
          finalize({
            turn,
            kind: "user",
            tag: TAG_BY_KIND.user,
            title,
            summary,
            detail: text7 || note
          })
        );
        break;
      }
      case "turn": {
        const group = ensureTurn(turn);
        if (item.streaming) group.streaming = true;
        if (item.report?.reason) group.reason = item.report.reason;
        const reported = item.report?.steps;
        if (reported && reported > group.steps) group.steps = reported;
        cells.push(...cellsFromTurn(item, turn));
        break;
      }
      case "notice":
        cells.push(
          finalize({
            turn,
            kind: "system",
            tag: TAG_BY_KIND.system,
            title: oneLine(item.text, 80),
            detail: item.text
          })
        );
        break;
      case "command": {
        const outcome = item.outcome === null ? "\u6267\u884C\u4E2D" : item.outcome.kind === "success" ? "\u5B8C\u6210" : "\u5931\u8D25";
        cells.push(
          finalize({
            turn,
            kind: "context",
            tag: TAG_BY_KIND.context,
            title: `/${item.name}${item.args}`,
            summary: `${outcome} \xB7 ${firstLine(item.outcome?.text ?? "", 60)}`,
            detail: item.outcome?.text ?? ""
          })
        );
        break;
      }
      case "compaction": {
        const title = item.error ? "\u538B\u7F29\u5931\u8D25" : item.running ? "\u4E0A\u4E0B\u6587\u538B\u7F29\u4E2D" : `\u538B\u7F29 \xB7 ${item.shadowedItemCount ?? "?"} \u6761`;
        cells.push(
          finalize({
            turn,
            kind: "context",
            tag: TAG_BY_KIND.context,
            title,
            summary: item.error ?? (item.summary ? firstLine(item.summary, 100) : void 0),
            detail: item.summary ?? item.error ?? ""
          })
        );
        break;
      }
    }
  });
  cells.forEach((cell, i) => {
    cell.index = i + 1;
    cell.id = `c${i + 1}`;
  });
  for (const cell of cells) {
    const group = ensureTurn(cell.turn);
    group.cells.push(cell);
    if (cell.kind === "tool") group.tools += 1;
    if (cell.ok === false) group.failed += 1;
  }
  const filled = turns.filter((group) => group.cells.length > 0);
  const usageByTurn = /* @__PURE__ */ new Map();
  let input2 = 0;
  let output = 0;
  let tools = 0;
  let failed = 0;
  ordinals.forEach((turn, i) => {
    const item = items[i];
    if (item.kind !== "turn") return;
    const usage = turnUsageOf(item);
    if (!usage) return;
    const acc = usageByTurn.get(turn) ?? { input: 0, output: 0 };
    acc.input += usage.inputTokens;
    acc.output += usage.outputTokens;
    usageByTurn.set(turn, acc);
  });
  for (const group of filled) {
    const usage = usageByTurn.get(group.turn);
    group.input = usage?.input ?? 0;
    group.output = usage?.output ?? 0;
    if (group.steps === 0) group.steps = group.tools;
    input2 += group.input;
    output += group.output;
    tools += group.tools;
    failed += group.failed;
  }
  return {
    cells,
    turns: filled,
    stats: { turns: filled.filter((g) => g.turn > 0).length, cells: cells.length, tools, input: input2, output, failed }
  };
}

// src/trajectory.tsx
import { jsx as jsx5, jsxs as jsxs4 } from "react/jsx-runtime";
var LANES = [
  { key: "input", label: "\u8F93\u5165", kinds: ["user", "system", "context"] },
  { key: "model", label: "\u6A21\u578B", kinds: ["message", "thinking", "error"] },
  { key: "tool", label: "\u5DE5\u5177", kinds: ["tool"] }
];
var KIND_CLASS = {
  user: "is-user",
  message: "is-message",
  thinking: "is-think",
  tool: "is-tool",
  system: "is-system",
  context: "is-context",
  error: "is-error"
};
function Chevron2({ open }) {
  return /* @__PURE__ */ jsx5(
    "svg",
    {
      width: "12",
      height: "12",
      viewBox: "0 0 24 24",
      fill: "none",
      stroke: "currentColor",
      strokeWidth: "2",
      strokeLinecap: "round",
      strokeLinejoin: "round",
      "aria-hidden": "true",
      style: { transform: open ? "rotate(90deg)" : void 0, transition: "transform 150ms ease" },
      children: /* @__PURE__ */ jsx5("path", { d: "M9 6l6 6-6 6" })
    }
  );
}
function Timeline({
  cells,
  stats,
  selectedId,
  onSelect
}) {
  if (cells.length === 0) return null;
  const span = cells[cells.length - 1].index || cells.length;
  const width = 100 / span;
  return /* @__PURE__ */ jsxs4("div", { className: "cc-trj-timeline", children: [
    /* @__PURE__ */ jsx5("p", { className: "cc-trj-statline", children: `${stats.turns} turns \xB7 ${stats.cells} records \xB7 ${stats.tools} tools \xB7 input \u2191${formatTokens(stats.input)} / output \u2193${formatTokens(stats.output)}` }),
    /* @__PURE__ */ jsxs4("div", { className: "cc-trj-lanes", children: [
      /* @__PURE__ */ jsx5("div", { className: "cc-trj-lane-labels", children: LANES.map((lane) => /* @__PURE__ */ jsx5("span", { className: "cc-trj-lane-label", children: lane.label }, lane.key)) }),
      /* @__PURE__ */ jsx5("div", { className: "cc-trj-lane-tracks", children: LANES.map((lane) => /* @__PURE__ */ jsx5("div", { className: "cc-trj-lane-track", children: cells.map(
        (cell) => lane.kinds.includes(cell.kind) ? /* @__PURE__ */ jsx5(
          "button",
          {
            type: "button",
            className: `cc-trj-block ${KIND_CLASS[cell.kind]}${selectedId === cell.id ? " on" : ""}`,
            style: { left: `${(cell.index - 1) * width}%`, width: `max(3px, ${width}%)` },
            title: `Turn ${cell.turn} \xB7 ${cell.tag} \xB7 ${cell.title}`,
            onClick: () => onSelect(cell.id)
          },
          cell.id
        ) : null
      ) }, lane.key)) })
    ] })
  ] });
}
function detailBody(cell) {
  return cell.detail?.trim() || "\uFF08\u65E0\u8BE6\u60C5\uFF09";
}
function Row({
  cell,
  open,
  selected,
  onToggle,
  registerRef
}) {
  const failed = cell.ok === false;
  return /* @__PURE__ */ jsxs4("div", { className: "cc-trj-rowwrap", children: [
    /* @__PURE__ */ jsxs4(
      "button",
      {
        type: "button",
        ref: registerRef,
        className: `cc-trj-row${selected ? " on" : ""}`,
        onClick: onToggle,
        "aria-expanded": open,
        children: [
          /* @__PURE__ */ jsx5("span", { className: `cc-trj-tag ${KIND_CLASS[cell.kind]}`, children: cell.tag }),
          /* @__PURE__ */ jsxs4("span", { className: "cc-trj-row-main", children: [
            /* @__PURE__ */ jsx5("span", { className: "cc-trj-row-title" + (cell.summary ? " with-sum" : ""), children: cell.title }),
            cell.summary && /* @__PURE__ */ jsx5("span", { className: "cc-trj-row-sum", children: cell.summary })
          ] }),
          /* @__PURE__ */ jsxs4("span", { className: "cc-trj-row-meta", children: [
            cell.tokens != null && cell.tokens > 0 && /* @__PURE__ */ jsx5("span", { children: `${formatTokens(cell.tokens)} tok` }),
            failed && /* @__PURE__ */ jsx5("span", { className: "is-fail", children: "\u5931\u8D25" }),
            !failed && cell.ok === true && /* @__PURE__ */ jsx5("span", { className: "is-ok", children: "\u5B8C\u6210" })
          ] }),
          /* @__PURE__ */ jsx5("span", { className: "cc-trj-row-caret", children: /* @__PURE__ */ jsx5(Chevron2, { open }) })
        ]
      }
    ),
    open && /* @__PURE__ */ jsx5("div", { className: "cc-trj-detail", children: /* @__PURE__ */ jsx5("pre", { children: detailBody(cell) }) })
  ] });
}
function TrajectoryView({ items }) {
  const model = useMemo(() => buildTrajectory(items), [items]);
  const [keyword, setKeyword] = useState5("");
  const [collapsed, setCollapsed] = useState5(() => /* @__PURE__ */ new Set());
  const [selectedId, setSelectedId] = useState5(null);
  const rowRefs = useRef3(/* @__PURE__ */ new Map());
  const kw = keyword.trim().toLowerCase();
  const view = useMemo(() => {
    if (!kw) return { cells: model.cells, turns: model.turns };
    const matched = model.cells.filter((cell) => cell.searchText.includes(kw));
    const ids = new Set(matched.map((cell) => cell.id));
    return {
      cells: matched,
      turns: model.turns.map((turn) => ({ ...turn, cells: turn.cells.filter((cell) => ids.has(cell.id)) })).filter((turn) => turn.cells.length > 0)
    };
  }, [model, kw]);
  const stats = useMemo(() => {
    const totals = { turns: view.turns.length, cells: view.cells.length, tools: 0, input: 0, output: 0, failed: 0 };
    for (const cell of view.cells) {
      if (cell.kind === "tool") totals.tools += 1;
      if (cell.ok === false) totals.failed += 1;
    }
    for (const turn of view.turns) {
      totals.input += turn.input;
      totals.output += turn.output;
    }
    return totals;
  }, [view]);
  const select = (cellId) => {
    setSelectedId(cellId);
    const cell = view.cells.find((item) => item.id === cellId);
    if (!cell) return;
    setCollapsed((prev) => {
      if (!prev.has(cell.turn)) return prev;
      const next = new Set(prev);
      next.delete(cell.turn);
      return next;
    });
  };
  useEffect4(() => {
    if (!selectedId) return;
    rowRefs.current.get(selectedId)?.scrollIntoView({ block: "center" });
  }, [selectedId, view.turns]);
  const allCollapsed = view.turns.length > 0 && view.turns.every((turn) => collapsed.has(turn.turn));
  const toggleAll = () => setCollapsed(allCollapsed ? /* @__PURE__ */ new Set() : new Set(view.turns.map((turn) => turn.turn)));
  if (model.cells.length === 0) {
    return /* @__PURE__ */ jsx5("div", { className: "cc-trj-root", children: /* @__PURE__ */ jsx5("div", { className: "cc-trj-empty", children: "\u8FD8\u6CA1\u6709\u53EF\u5C55\u793A\u7684\u8F68\u8FF9\u8BB0\u5F55" }) });
  }
  return /* @__PURE__ */ jsxs4("div", { className: "cc-trj-root", children: [
    /* @__PURE__ */ jsxs4("div", { className: "cc-trj-toolbar", children: [
      /* @__PURE__ */ jsxs4("span", { className: "cc-trj-searchbox", children: [
        /* @__PURE__ */ jsxs4(
          "svg",
          {
            width: "12",
            height: "12",
            viewBox: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            strokeWidth: "2",
            strokeLinecap: "round",
            strokeLinejoin: "round",
            "aria-hidden": "true",
            children: [
              /* @__PURE__ */ jsx5("circle", { cx: "11", cy: "11", r: "7" }),
              /* @__PURE__ */ jsx5("path", { d: "m20 20-3.5-3.5" })
            ]
          }
        ),
        /* @__PURE__ */ jsx5(
          "input",
          {
            value: keyword,
            onChange: (e) => setKeyword(e.target.value),
            placeholder: "\u641C\u7D22\u8BB0\u5F55\u2026",
            "aria-label": "\u641C\u7D22\u8F68\u8FF9\u8BB0\u5F55"
          }
        )
      ] }),
      /* @__PURE__ */ jsx5("span", { className: "cc-trj-count", children: `${view.cells.length} \u6761` }),
      /* @__PURE__ */ jsx5("button", { type: "button", className: "cc-trj-toggleall", onClick: toggleAll, children: allCollapsed ? "\u5C55\u5F00\u5168\u90E8" : "\u6298\u53E0\u5168\u90E8" })
    ] }),
    /* @__PURE__ */ jsx5(Timeline, { cells: view.cells, stats, selectedId, onSelect: select }),
    /* @__PURE__ */ jsx5("div", { className: "cc-trj-table", children: view.turns.length === 0 ? /* @__PURE__ */ jsx5("p", { className: "cc-trj-empty", children: "\u6CA1\u6709\u5339\u914D\u7684\u8BB0\u5F55" }) : view.turns.map((turn) => {
      const isCollapsed = collapsed.has(turn.turn);
      const tokens = turn.input + turn.output;
      return /* @__PURE__ */ jsxs4("section", { className: "cc-trj-turn", children: [
        /* @__PURE__ */ jsxs4(
          "button",
          {
            type: "button",
            className: "cc-trj-turnhead" + (turn.streaming ? " run" : ""),
            onClick: () => setCollapsed((prev) => {
              const next = new Set(prev);
              if (next.has(turn.turn)) next.delete(turn.turn);
              else next.add(turn.turn);
              return next;
            }),
            "aria-expanded": !isCollapsed,
            children: [
              /* @__PURE__ */ jsx5("span", { className: "cc-trj-chev", children: /* @__PURE__ */ jsx5(Chevron2, { open: !isCollapsed }) }),
              /* @__PURE__ */ jsx5("span", { className: "cc-trj-turnname", children: turn.turn === 0 ? "\u4F1A\u8BDD\u8BB0\u5F55" : `Turn ${turn.turn}` }),
              turn.reason && /* @__PURE__ */ jsx5("span", { className: "cc-trj-reason", children: turn.reason }),
              turn.streaming && /* @__PURE__ */ jsx5("span", { className: "cc-trj-live", children: "\u8FDB\u884C\u4E2D" }),
              /* @__PURE__ */ jsx5("span", { className: "cc-trj-turnmeta", children: `${turn.steps} steps \xB7 ${turn.tools} tools${tokens > 0 ? ` \xB7 ${formatTokens(tokens)} tokens` : ""}` })
            ]
          }
        ),
        !isCollapsed && turn.cells.map((cell) => /* @__PURE__ */ jsx5(
          Row,
          {
            cell,
            open: selectedId === cell.id,
            selected: selectedId === cell.id,
            onToggle: () => selectedId === cell.id ? setSelectedId(null) : select(cell.id),
            registerRef: (el) => {
              if (el) rowRefs.current.set(cell.id, el);
              else rowRefs.current.delete(cell.id);
            }
          },
          cell.id
        ))
      ] }, turn.turn);
    }) })
  ] });
}

// src/workspace-picker.tsx
import { useEffect as useEffect5, useRef as useRef4, useState as useState6 } from "react";
import { Fragment as Fragment5, jsx as jsx6, jsxs as jsxs5 } from "react/jsx-runtime";
function WorkspacePicker({ agent, startDir, recent = [], native, onCancel, onPick }) {
  const [listing, setListing] = useState6(null);
  const [error, setError] = useState6(null);
  const [loading, setLoading] = useState6(true);
  const [nativeBusy, setNativeBusy] = useState6(false);
  const aliveRef = useRef4(true);
  const navigate = (path2) => {
    setLoading(true);
    agent.listDirs(path2).then((data) => {
      if (!aliveRef.current) return;
      setListing(data);
      setError(null);
    }).catch((e) => {
      if (!aliveRef.current) return;
      setError(e instanceof Error ? e.message : String(e));
    }).finally(() => {
      if (aliveRef.current) setLoading(false);
    });
  };
  useEffect5(() => {
    aliveRef.current = true;
    navigate(startDir);
    return () => {
      aliveRef.current = false;
    };
  }, [startDir]);
  useEffect5(() => {
    const onKey = (e) => {
      if (e.key === "Escape") onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onCancel]);
  const browseNative = async () => {
    if (!native || nativeBusy) return;
    setNativeBusy(true);
    try {
      const dir = await native();
      if (dir) onPick(dir);
    } finally {
      if (aliveRef.current) setNativeBusy(false);
    }
  };
  return /* @__PURE__ */ jsxs5(Fragment5, { children: [
    /* @__PURE__ */ jsx6("div", { className: "cc-picker-mask", onClick: onCancel }),
    /* @__PURE__ */ jsxs5("div", { className: "cc-picker", role: "dialog", "aria-modal": "true", "aria-label": "\u9009\u62E9\u5DE5\u4F5C\u76EE\u5F55", children: [
      /* @__PURE__ */ jsxs5("header", { className: "cc-picker-head", children: [
        /* @__PURE__ */ jsx6("span", { className: "cc-picker-title", children: "\u9009\u62E9\u5DE5\u4F5C\u76EE\u5F55" }),
        /* @__PURE__ */ jsx6("button", { className: "cc-picker-x", onClick: onCancel, title: "\u5173\u95ED", "aria-label": "\u5173\u95ED", children: "\u2715" })
      ] }),
      /* @__PURE__ */ jsx6("div", { className: "cc-picker-path", title: listing?.path ?? "", children: listing?.path ?? (loading ? "\u8F7D\u5165\u4E2D\u2026" : "") }),
      /* @__PURE__ */ jsxs5("div", { className: "cc-picker-list", children: [
        listing?.parent && /* @__PURE__ */ jsxs5("button", { className: "cc-picker-row", onClick: () => navigate(listing.parent ?? void 0), children: [
          /* @__PURE__ */ jsx6("span", { className: "cc-picker-ic", children: "\u2191" }),
          /* @__PURE__ */ jsx6("span", { className: "cc-picker-name", children: "\u4E0A\u4E00\u7EA7" })
        ] }),
        listing?.entries.map((entry) => /* @__PURE__ */ jsxs5(
          "button",
          {
            className: "cc-picker-row",
            title: entry.path,
            onClick: () => navigate(entry.path),
            children: [
              /* @__PURE__ */ jsx6("span", { className: "cc-picker-ic", children: "\u{1F4C1}" }),
              /* @__PURE__ */ jsx6("span", { className: "cc-picker-name", children: entry.name })
            ]
          },
          entry.path
        )),
        listing && listing.entries.length === 0 && /* @__PURE__ */ jsx6("div", { className: "cc-picker-empty", children: "\u8BE5\u76EE\u5F55\u4E0B\u6CA1\u6709\u5B50\u76EE\u5F55" })
      ] }),
      recent.length > 0 && /* @__PURE__ */ jsxs5("div", { className: "cc-picker-recent", children: [
        /* @__PURE__ */ jsx6("span", { className: "cc-picker-recent-label", children: "\u6700\u8FD1\u4F7F\u7528" }),
        recent.map((dir) => /* @__PURE__ */ jsx6("button", { className: "cc-picker-recent-item", title: dir, onClick: () => onPick(dir), children: dir }, dir))
      ] }),
      listing && listing.roots.length > 0 && /* @__PURE__ */ jsx6("div", { className: "cc-picker-roots", children: listing.roots.map((root4) => /* @__PURE__ */ jsx6("button", { className: "cc-picker-root", onClick: () => navigate(root4), children: root4 }, root4)) }),
      error && /* @__PURE__ */ jsx6("div", { className: "cc-picker-err", children: error }),
      /* @__PURE__ */ jsxs5("footer", { className: "cc-picker-foot", children: [
        native && /* @__PURE__ */ jsx6(
          "button",
          {
            className: "cc-picker-native",
            disabled: nativeBusy,
            title: "\u4F7F\u7528\u64CD\u4F5C\u7CFB\u7EDF\u6587\u4EF6\u5939\u9009\u62E9\u5668",
            onClick: () => void browseNative(),
            children: nativeBusy ? "\u6253\u5F00\u7CFB\u7EDF\u5BF9\u8BDD\u6846\u2026" : "\u6D4F\u89C8\u7CFB\u7EDF\u76EE\u5F55\u2026"
          }
        ),
        /* @__PURE__ */ jsx6("span", { className: "cc-picker-foot-gap" }),
        /* @__PURE__ */ jsx6("button", { className: "cc-picker-cancel", onClick: onCancel, children: "\u53D6\u6D88" }),
        /* @__PURE__ */ jsx6(
          "button",
          {
            className: "cc-picker-ok",
            disabled: !listing || loading,
            onClick: () => listing && onPick(listing.path),
            children: "\u4F7F\u7528\u6B64\u76EE\u5F55"
          }
        )
      ] })
    ] })
  ] });
}

// src/session-store.ts
import { useSyncExternalStore } from "react";
var state = { sessions: [], currentId: null, recents: [], command: null };
var listeners = /* @__PURE__ */ new Set();
var seq = 0;
function publish(patch2) {
  state = { ...state, ...patch2 };
  for (const listener of listeners) listener();
}
var chatStore = {
  subscribe(listener) {
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  },
  snapshot() {
    return state;
  },
  /** 写入会话列表并返回排序后的副本（调用方据此决定「启动恢复哪个会话」） */
  setSessions(sessions) {
    const visible = sessions.filter((session) => session.eventCount > 0 || session.id === state.currentId);
    const sorted = [...visible].sort((a, b) => (b.updatedAtMs ?? 0) - (a.updatedAtMs ?? 0));
    publish({ sessions: sorted });
    return sorted;
  },
  setCurrent(currentId) {
    publish({ currentId });
  },
  /** 就地更新单个会话标题（自动命名 durable `session_title` 事件到达时） */
  setTitle(id, title) {
    const target = state.sessions.find((session) => session.id === id);
    if (!target || target.title === title) return;
    publish({
      sessions: state.sessions.map(
        (session) => session.id === id ? { ...session, title } : session
      )
    });
  },
  setRecents(recents) {
    publish({ recents });
  },
  /** 请求切到某会话 */
  select(id) {
    publish({ command: { seq: ++seq, kind: "select", id } });
  },
  /** 请求新建会话（可选工作区绝对路径） */
  create(cwd2) {
    publish({ command: { seq: ++seq, kind: "new", cwd: cwd2 } });
  }
};
function useChatStore() {
  return useSyncExternalStore(chatStore.subscribe, chatStore.snapshot);
}
function baseName2(path2) {
  const parts = path2.split(/[\\/]/).filter(Boolean);
  return parts.length > 0 ? parts[parts.length - 1] : path2;
}
function groupSessions(sessions) {
  const buckets = /* @__PURE__ */ new Map();
  for (const session of sessions) {
    const key = session.workingDir ?? "";
    const bucket = buckets.get(key);
    if (bucket) bucket.push(session);
    else buckets.set(key, [session]);
  }
  return [...buckets].map(([key, list4]) => ({
    key,
    label: key ? baseName2(key) : "\u672A\u5F52\u7C7B",
    sessions: list4
  }));
}
function sessionLabel(session) {
  const title = session.title.trim();
  return title || `\u65B0\u4F1A\u8BDD ${session.id.slice(-6)}`;
}
function sessionTime(updatedAtMs) {
  if (!updatedAtMs) return "";
  const delta = Date.now() - updatedAtMs;
  if (delta < 6e4) return "\u521A\u521A";
  if (delta < 36e5) return `${Math.floor(delta / 6e4)} \u5206\u949F\u524D`;
  if (delta < 864e5) return `${Math.floor(delta / 36e5)} \u5C0F\u65F6\u524D`;
  return `${Math.floor(delta / 864e5)} \u5929\u524D`;
}

// src/export.ts
function stepLine(name2, isError, summary) {
  const tail = summary ? `\uFF1A${summary}` : "";
  return `- ${isError ? "\u5931\u8D25 " : ""}${name2}${tail}`;
}
function itemsToMarkdown(items) {
  const blocks = [];
  for (const item of items) {
    if (item.kind === "user") {
      blocks.push(`## \u6211

${item.text.trim()}`);
      continue;
    }
    if (item.kind === "command") {
      const outcome = item.outcome?.text.trim() ?? "";
      blocks.push(`> \`/${item.name}${item.args}\`${outcome ? `
>
> ${outcome.replace(/\n/g, "\n> ")}` : ""}`);
      continue;
    }
    if (item.kind === "compaction") {
      const counts = item.shadowedItemCount !== null && item.shadowedTokenCount !== null ? `\uFF08\u5DF2\u538B\u7F29 ${item.shadowedItemCount} \u6761\u5386\u53F2\u8BB0\u5F55\uFF0C\u7EA6 ${item.shadowedTokenCount} tokens\uFF09` : "";
      blocks.push(`> \u4E0A\u4E0B\u6587\u5DF2\u538B\u7F29${counts}`);
      continue;
    }
    if (item.kind !== "turn") continue;
    if (item.steps.length > 0) {
      const lines = item.steps.map((s) => stepLine(s.name, s.isError, firstLine(s.resultText ?? s.inputText ?? "", 120)));
      blocks.push(`> \u5DE5\u5177\u8C03\u7528
>
${lines.map((l) => `> ${l}`).join("\n")}`);
    }
    if (item.answer.trim()) blocks.push(`## \u52A9\u624B

${item.answer.trim()}`);
  }
  return blocks.join("\n\n");
}
async function sessionToMarkdown(agent, session) {
  const events = await agent.getMessages(session.id);
  const items = framesToItems(events);
  const body = itemsToMarkdown(items);
  const head = [
    `# ${sessionLabel(session)}`,
    "",
    `- \u4F1A\u8BDD\uFF1A${session.id}`,
    `- \u5DE5\u4F5C\u533A\uFF1A${session.workingDir ?? "\uFF08\u672A\u6307\u5B9A\uFF09"}`,
    `- \u6700\u8FD1\u6D3B\u8DC3\uFF1A${sessionTime(session.updatedAtMs) || "\u672A\u77E5"}`,
    `- \u5BFC\u51FA\u65F6\u95F4\uFF1A${(/* @__PURE__ */ new Date()).toLocaleString("zh-CN")}`,
    "",
    "---",
    ""
  ].join("\n");
  return `${head}${body || "\uFF08\u8BE5\u4F1A\u8BDD\u6682\u65E0\u6D88\u606F\uFF09"}
`;
}
function downloadText(fileName, text7) {
  const safe = fileName.replace(/[\\/:*?"<>|]+/g, "_").slice(0, 60) || "session";
  const blob = new Blob([text7], { type: "text/markdown;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const link3 = document.createElement("a");
  link3.href = url;
  link3.download = `${safe}.md`;
  document.body.appendChild(link3);
  link3.click();
  link3.remove();
  URL.revokeObjectURL(url);
}

// src/chat-view.tsx
import { jsx as jsx7, jsxs as jsxs6 } from "react/jsx-runtime";
function errText(e) {
  return e instanceof Error ? e.message : String(e);
}
var MAX_IMAGE_BYTES = 4 * 1024 * 1024;
var MAX_ATTACHMENTS = 6;
function readImageAttachment(file) {
  return new Promise((resolve, reject) => {
    if (!file.type.startsWith("image/")) {
      reject(new Error(`\u4EC5\u652F\u6301\u56FE\u7247\u9644\u4EF6\uFF1A${file.name}`));
      return;
    }
    if (file.size > MAX_IMAGE_BYTES) {
      reject(new Error(`\u56FE\u7247\u8D85\u8FC7 4MB\uFF1A${file.name}`));
      return;
    }
    const reader = new FileReader();
    reader.onload = () => resolve({ name: file.name, mimeType: file.type, dataUrl: String(reader.result ?? "") });
    reader.onerror = () => reject(new Error(`\u8BFB\u53D6\u5931\u8D25\uFF1A${file.name}`));
    reader.readAsDataURL(file);
  });
}
var ICON2 = {
  target: "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18z M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8z",
  checklist: "M3 6l1.5 1.5L7 5 M3 13l1.5 1.5L7 12 M3 20l1.5 1.5L7 19 M10 6h11 M10 13h11 M10 20h11",
  plan: "M5 4h9l5 5v11H5z M14 4v5h5 M8 13h8 M8 17h5",
  pause: "M9 5v14 M15 5v14",
  resume: "M8 5l11 7-11 7z",
  close: "M6 6l12 12 M18 6L6 18",
  check: "M5 12l5 5L19 7",
  // 目标清除动作用垃圾桶图标（IconTrashOutlineRegular）
  trash: "M4 7h16 M9 7V5h6v2 M6 7l1 13h10l1-13 M10 11v6 M14 11v6",
  closeCircle: "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18z M9 9l6 6 M15 9l-6 6",
  edit: "M4 20h4l10-10-4-4L4 16z M14 6l4 4",
  chevron: "M6 9l6 6 6-6",
  chevronUp: "M6 15l6-6 6 6",
  attach: "M12 5v14 M5 12h14",
  image: "M4 5h16v14H4z M8.5 11a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3z M4 16l5-5 4 4 3-3 4 4",
  terminal: "M4 5h16v14H4z M8 10l2 2-2 2 M13 14h3"
};
var APPROVAL_PRESETS = ["workspace-write", "auto", "danger-full-access"];
function goalFromEvent(event) {
  const snap = event?.snapshot;
  if (!snap || typeof snap.id !== "string" || typeof snap.objective !== "string") return null;
  const reason = snap.blocked_reason ?? snap.blockedReason;
  return {
    id: snap.id,
    revision: Number(snap.revision ?? 0),
    objective: snap.objective,
    phase: String(snap.phase ?? "active"),
    blockedReason: reason && typeof reason === "object" ? { code: String(reason.code ?? ""), message: String(reason.message ?? "") } : void 0,
    maxGoalRounds: Number(snap.max_goal_rounds ?? snap.maxGoalRounds ?? 0),
    roundsStarted: Number(snap.rounds_started ?? snap.roundsStarted ?? 0),
    createdAtMs: Number(snap.created_at_ms ?? snap.createdAtMs ?? 0),
    updatedAtMs: Number(snap.updated_at_ms ?? snap.updatedAtMs ?? 0),
    armed: snap.armed === true
  };
}
function planFromEvent(event) {
  return { active: event?.active === true, pending: null };
}
var SLASH_COMMANDS = [
  { name: "goal", args: "[<\u76EE\u6807>|clear|edit <\u76EE\u6807>|pause|resume]", label: "\u76EE\u6807", desc: "\u8BBE\u7F6E\u6216\u67E5\u770B\u957F\u671F\u4EFB\u52A1\u76EE\u6807", section: "add", icon: ICON2.target },
  { name: "plan", args: "[off|\u6D88\u606F]", label: "\u8BA1\u5212", desc: "\u8FDB\u5165\u6216\u9000\u51FA\u8BA1\u5212\u6A21\u5F0F", section: "add", icon: ICON2.plan },
  { name: "compact", label: "\u538B\u7F29", desc: "\u538B\u7F29\u4EE5\u4E0A\u5BF9\u8BDD\u5185\u5BB9", section: "commands", icon: ICON2.terminal },
  { name: "permission", args: "[<\u9884\u8BBE>]", label: "\u6743\u9650", desc: "\u67E5\u770B\u6216\u5207\u6362\u5BA1\u6279\u9884\u8BBE", section: "commands", icon: ICON2.terminal },
  { name: "feedback", args: "<\u6587\u672C>", label: "\u53CD\u9988", desc: "\u8BB0\u5F55\u4E00\u6761\u4F1A\u8BDD\u53CD\u9988", section: "commands", icon: ICON2.terminal },
  { name: "export", label: "\u5BFC\u51FA", desc: "\u628A\u672C\u4F1A\u8BDD\u5BFC\u51FA\u4E3A Markdown \u6587\u4EF6", section: "commands", icon: ICON2.terminal }
];
var SECTION_LABELS = {
  add: "\u6DFB\u52A0",
  commands: "\u6307\u4EE4"
};
var COMMAND_ALIASES = {
  \u76EE\u6807: "goal",
  \u8BA1\u5212: "plan",
  \u53CD\u9988: "feedback",
  \u538B\u7F29: "compact",
  \u6743\u9650: "permission",
  \u5BFC\u51FA: "export"
};
var HELP_TEXT = [
  "/compact \u2014 \u538B\u7F29\u4E0A\u4E0B\u6587\uFF08\u522B\u540D /\u538B\u7F29\uFF09",
  "/goal <\u76EE\u6807> \u2014 \u521B\u5EFA/\u7F16\u8F91\u76EE\u6807\uFF1B/goal pause|resume|complete|block|clear \u64CD\u4F5C\uFF08\u522B\u540D /\u76EE\u6807\uFF09",
  "/plan [off|\u6D88\u606F] \u2014 \u8FDB\u5165 plan \u6A21\u5F0F\uFF08\u53EF\u5E26\u5F15\u5BFC\u6D88\u606F\uFF09\uFF1B/plan off \u9000\u51FA\uFF08\u522B\u540D /\u8BA1\u5212\uFF09",
  "/permission [<\u9884\u8BBE>] \u2014 \u67E5\u770B\u6216\u5207\u6362\u5BA1\u6279\u9884\u8BBE\uFF08workspace-write / auto / danger-full-access\uFF09\uFF08\u522B\u540D /\u6743\u9650\uFF09",
  "/feedback <\u6587\u672C> \u2014 \u8BB0\u5F55\u4E00\u6761\u4F1A\u8BDD\u53CD\u9988\uFF08\u522B\u540D /\u53CD\u9988\uFF09",
  "/export \u2014 \u628A\u672C\u4F1A\u8BDD\u5BFC\u51FA\u4E3A Markdown \u6587\u4EF6\uFF08\u522B\u540D /\u5BFC\u51FA\uFF09"
].join("\n");
function GoalBar({
  goal,
  onAction,
  onEdit
}) {
  const [editing, setEditing] = useState7(false);
  const [draft, setDraft] = useState7("");
  const phase = goal.phase === "active" ? goal.armed ? "\u8FDB\u884C\u4E2D\u7684\u76EE\u6807" : "\u5F85\u7EE7\u7EED\u7684\u76EE\u6807" : goal.phase === "paused" ? "\u5DF2\u6682\u505C\u7684\u76EE\u6807" : goal.phase === "blocked" ? "\u53D7\u963B\u7684\u76EE\u6807" : "\u8FDB\u884C\u4E2D\u7684\u76EE\u6807";
  const blocked = goal.phase === "blocked" ? goal.blockedReason?.message : void 0;
  const resumable = goal.phase !== "active" || !goal.armed;
  const save = () => {
    const trimmed = draft.trim();
    if (trimmed === "") return;
    onEdit(trimmed);
    setEditing(false);
  };
  if (editing) {
    return /* @__PURE__ */ jsxs6("div", { className: "cc-goalbar is-" + goal.phase, children: [
      /* @__PURE__ */ jsx7("span", { className: "cc-goal-glyph", children: /* @__PURE__ */ jsx7(Icon, { d: ICON2.target, size: 14 }) }),
      /* @__PURE__ */ jsx7(
        "input",
        {
          className: "cc-goal-input",
          value: draft,
          autoFocus: true,
          "aria-label": "\u76EE\u6807\u5185\u5BB9",
          onChange: (e) => setDraft(e.target.value),
          onKeyDown: (e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              save();
            } else if (e.key === "Escape") {
              e.preventDefault();
              setEditing(false);
            }
          }
        }
      ),
      /* @__PURE__ */ jsxs6("span", { className: "cc-goal-actions", children: [
        /* @__PURE__ */ jsx7("button", { className: "cc-goal-btn", onClick: save, disabled: draft.trim() === "", title: "\u4FDD\u5B58", children: /* @__PURE__ */ jsx7(Icon, { d: ICON2.check, size: 14 }) }),
        /* @__PURE__ */ jsx7("button", { className: "cc-goal-btn", onClick: () => setEditing(false), title: "\u53D6\u6D88", children: /* @__PURE__ */ jsx7(Icon, { d: ICON2.close, size: 14 }) })
      ] })
    ] });
  }
  return /* @__PURE__ */ jsxs6(
    "div",
    {
      className: "cc-goalbar is-" + goal.phase,
      title: blocked || `${goal.objective}\uFF08\u7B2C ${goal.roundsStarted}/${goal.maxGoalRounds} \u8F6E\uFF09`,
      children: [
        /* @__PURE__ */ jsx7("span", { className: "cc-goal-glyph", children: /* @__PURE__ */ jsx7(Icon, { d: ICON2.target, size: 14 }) }),
        /* @__PURE__ */ jsx7("span", { className: "cc-goal-label", children: phase }),
        /* @__PURE__ */ jsx7("span", { className: "cc-goal-text", children: goal.objective }),
        /* @__PURE__ */ jsxs6("span", { className: "cc-goal-actions", children: [
          /* @__PURE__ */ jsx7(
            "button",
            {
              className: "cc-goal-btn",
              onClick: () => {
                setDraft(goal.objective);
                setEditing(true);
              },
              title: "\u7F16\u8F91\u76EE\u6807",
              children: /* @__PURE__ */ jsx7(Icon, { d: ICON2.edit, size: 14 })
            }
          ),
          /* @__PURE__ */ jsx7("button", { className: "cc-goal-btn", onClick: () => onAction("clear"), title: "\u6E05\u9664\u76EE\u6807", children: /* @__PURE__ */ jsx7(Icon, { d: ICON2.trash, size: 14 }) }),
          resumable ? /* @__PURE__ */ jsx7("button", { className: "cc-goal-btn", onClick: () => onAction("resume"), title: "\u7EE7\u7EED\u76EE\u6807", children: /* @__PURE__ */ jsx7(Icon, { d: ICON2.resume, size: 14 }) }) : /* @__PURE__ */ jsx7("button", { className: "cc-goal-btn", onClick: () => onAction("pause"), title: "\u6682\u505C\u76EE\u6807", children: /* @__PURE__ */ jsx7(Icon, { d: ICON2.pause, size: 14 }) })
        ] })
      ]
    }
  );
}
function TodoStateDot({ status }) {
  if (status === "in_progress") {
    return /* @__PURE__ */ jsx7("span", { className: "cc-statedot is-ongoing", "aria-label": "\u8FDB\u884C\u4E2D", children: /* @__PURE__ */ jsxs6("svg", { viewBox: "0 0 16 16", "aria-hidden": "true", children: [
      /* @__PURE__ */ jsx7("circle", { className: "cc-statedot-track", cx: "8", cy: "8", r: "6" }),
      /* @__PURE__ */ jsx7("circle", { className: "cc-statedot-arc", cx: "8", cy: "8", r: "6" })
    ] }) });
  }
  return /* @__PURE__ */ jsx7("span", { className: "cc-statedot " + (status === "completed" ? "is-done" : "is-idle"), "aria-hidden": "true" });
}
function todoProgress(todos) {
  const done = todos.filter((t) => t.status === "completed").length;
  const active = todos.filter((t) => t.status === "in_progress").length;
  const pending = todos.length - done - active;
  return [
    done > 0 ? `${done} \u5DF2\u5B8C\u6210` : "",
    active > 0 ? `${active} \u8FDB\u884C\u4E2D` : "",
    pending > 0 ? `${pending} \u5F85\u5904\u7406` : ""
  ].filter(Boolean).join("\u2002\xB7\u2002");
}
function TodoPanel({ todos }) {
  const [collapsed, setCollapsed] = useState7(true);
  if (todos.length === 0) return null;
  return /* @__PURE__ */ jsx7("div", { className: "cc-todopanel", children: /* @__PURE__ */ jsxs6("div", { className: "cc-todopanel-body", children: [
    /* @__PURE__ */ jsxs6("button", { className: "cc-todopanel-head", onClick: () => setCollapsed((v) => !v), "aria-expanded": !collapsed, children: [
      /* @__PURE__ */ jsx7("span", { className: "cc-todopanel-glyph", children: /* @__PURE__ */ jsx7(Icon, { d: ICON2.checklist, size: 14 }) }),
      /* @__PURE__ */ jsx7("span", { className: "cc-todopanel-title", children: "\u4EFB\u52A1" }),
      /* @__PURE__ */ jsx7("span", { className: "cc-todopanel-progress", children: todoProgress(todos) }),
      /* @__PURE__ */ jsx7("span", { className: "cc-chevron", children: /* @__PURE__ */ jsx7(Icon, { d: collapsed ? ICON2.chevronUp : ICON2.chevron }) })
    ] }),
    !collapsed && /* @__PURE__ */ jsx7("ul", { className: "cc-todopanel-list", children: todos.map((t, i) => /* @__PURE__ */ jsxs6("li", { className: "cc-todopanel-item", "data-status": t.status, children: [
      /* @__PURE__ */ jsx7("span", { className: "cc-todopanel-dot", children: /* @__PURE__ */ jsx7(TodoStateDot, { status: t.status }) }),
      /* @__PURE__ */ jsx7("span", { className: "cc-todopanel-text", children: t.content })
    ] }, i)) })
  ] }) });
}
function PlanChip({ plan, onExit }) {
  const effective = plan.pending !== null && plan.pending !== void 0 ? !plan.active : plan.active;
  if (!effective) return null;
  return /* @__PURE__ */ jsxs6(
    "button",
    {
      className: "cc-planchip" + (plan.pending !== null && plan.pending !== void 0 ? " is-pending" : ""),
      onClick: onExit,
      title: plan.pending !== null && plan.pending !== void 0 ? "plan \u6A21\u5F0F\u5207\u6362\u4E2D\uFF08\u5F53\u524D\u8F6E\u6B21\u7ED3\u675F\u540E\u751F\u6548\uFF09" : "\u70B9\u51FB\u9000\u51FA plan \u6A21\u5F0F",
      children: [
        /* @__PURE__ */ jsx7("span", { className: "cc-planchip-ico", children: /* @__PURE__ */ jsx7(Icon, { d: ICON2.plan, size: 14 }) }),
        /* @__PURE__ */ jsx7("span", { className: "cc-planchip-close", children: /* @__PURE__ */ jsx7(Icon, { d: ICON2.closeCircle, size: 14 }) }),
        "\u8BA1\u5212\u6A21\u5F0F"
      ]
    }
  );
}
function normalizeQuestions(raw) {
  const list4 = Array.isArray(raw) ? raw : [];
  return list4.map((q) => {
    const obj = typeof q === "object" && q !== null ? q : {};
    return {
      id: obj.id,
      question: obj.question,
      header: obj.header,
      options: obj.options,
      multiSelect: obj.multiSelect === true || obj.multi_select === true
    };
  });
}
function pendingFromFrame(frame) {
  const requestId = typeof frame.requestId === "string" ? frame.requestId : "";
  if (!requestId) return null;
  switch (frame.type) {
    case "approval_requested":
      return {
        requestId,
        kind: "approval",
        tool: frame.tool,
        input: frame.input,
        reason: frame.reason
      };
    case "plan_review_requested":
      return { requestId, kind: "plan_review", plan: frame.plan };
    case "question_requested":
      return {
        requestId,
        kind: "question",
        callId: frame.callId,
        questions: normalizeQuestions(frame.questions)
      };
    default:
      return null;
  }
}
function pendingFromRest(item) {
  return {
    requestId: item.requestId,
    kind: item.kind,
    tool: item.tool,
    input: item.input,
    reason: item.reason,
    plan: item.plan,
    callId: item.callId,
    questions: item.questions
  };
}
function ContextMeter({ tokens, limit }) {
  const pct = limit > 0 ? Math.min(100, Math.round(tokens / limit * 100)) : 0;
  const level = pct >= 90 ? " danger" : pct >= 70 ? " warn" : "";
  return /* @__PURE__ */ jsxs6(
    "div",
    {
      className: "cc-ctx" + level,
      title: `\u4E0A\u4E0B\u6587\u5360\u7528 ${tokens.toLocaleString()} / ${limit.toLocaleString()} tokens\uFF08chars/4 \u4F30\u7B97\uFF09`,
      children: [
        /* @__PURE__ */ jsx7("span", { className: "cc-ctx-bar", children: /* @__PURE__ */ jsx7("i", { style: { width: `${pct}%` } }) }),
        /* @__PURE__ */ jsxs6("span", { className: "cc-ctx-text", children: [
          "\u4E0A\u4E0B\u6587 ",
          pct,
          "%",
          limit > 0 ? ` \xB7 ${formatTokens(tokens)}/${formatTokens(limit)}` : ""
        ] })
      ]
    }
  );
}
function ChatView({ agent, storage, pickNative }) {
  const [items, setItems] = useState7([]);
  const [pending, setPending] = useState7([]);
  const [input2, setInput] = useState7("");
  const [attachments, setAttachments] = useState7([]);
  const [busy, setBusy] = useState7(false);
  const [error, setError] = useState7(null);
  const { currentId: sessionId, recents: workspaces, command } = useChatStore();
  const [info, setInfo] = useState7(null);
  const [catalog, setCatalog] = useState7([]);
  const [wsPicker, setWsPicker] = useState7(false);
  const [lastTurn, setLastTurn] = useState7(null);
  const [view, setView] = useState7("chat");
  const [mode, setMode] = useState7("standard");
  const [draftCwd, setDraftCwd] = useState7(void 0);
  const [draftModel, setDraftModel] = useState7("");
  const [goal, setGoal] = useState7(null);
  const [plan, setPlan] = useState7(null);
  const [todos, setTodos] = useState7([]);
  const [slashIndex, setSlashIndex] = useState7(0);
  const [slashDismissed, setSlashDismissed] = useState7(false);
  const busyRef = useRef5(false);
  const sessionRef = useRef5(null);
  const subRef = useRef5(null);
  const scrollRef = useRef5(null);
  const inputRef = useRef5(null);
  const fileRef = useRef5(null);
  const slashListRef = useRef5(null);
  const addFiles = async (files) => {
    const next = [];
    const errs = [];
    for (const file of Array.from(files)) {
      try {
        next.push(await readImageAttachment(file));
      } catch (e) {
        errs.push(errText(e));
      }
    }
    if (next.length > 0) {
      if (attachments.length + next.length > MAX_ATTACHMENTS) {
        errs.push(`\u5355\u6761\u6D88\u606F\u6700\u591A ${MAX_ATTACHMENTS} \u5F20\u56FE\u7247`);
      }
      setAttachments((prev) => [...prev, ...next].slice(0, MAX_ATTACHMENTS));
    }
    if (errs.length > 0) setError(errs.join("\uFF1B"));
  };
  useEffect6(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [items, pending]);
  useEffect6(() => {
    if (!lastTurn) return;
    setItems((prev) => {
      const last = prev[prev.length - 1];
      if (!last || last.kind !== "turn" || last.streaming || last.report) return prev;
      if (last.turn !== lastTurn.turn) return prev;
      return [...prev.slice(0, -1), { ...last, report: lastTurn }];
    });
  }, [items, lastTurn]);
  const handleFrame = (frame) => {
    const card = pendingFromFrame(frame);
    if (card) {
      setPending(
        (prev) => prev.some((p) => p.requestId === card.requestId) ? prev : [...prev, card]
      );
      return;
    }
    if (frame.type === "session_closed") {
      busyRef.current = false;
      setBusy(false);
      return;
    }
    if (frame.type === "durable") {
      const event = frame.event;
      switch (event?.type) {
        // 自动命名结果落盘即广播（durable `session_title`）→ 侧边栏就地换标题，
        // 无需轮询；服务端标题生成是轮次结束后的异步模型调用。
        case "session_title":
          if (typeof event.title === "string") chatStore.setTitle(sessionRef.current ?? "", event.title);
          break;
        case "todo_write":
          setTodos(todosFromEvent(event));
          break;
        case "goal_change": {
          const next = goalFromEvent(event);
          setGoal(next);
          const id = sessionRef.current;
          if (next && id) {
            agent.getGoal(id).then((fresh) => {
              if (sessionRef.current === id) setGoal(fresh);
            }).catch(() => void 0);
          }
          break;
        }
        case "plan_mode":
          setPlan(planFromEvent(event));
          break;
        case "plan_wanted":
          setPlan((prev) => ({ active: prev?.active ?? false, pending: event.active === true }));
          break;
      }
    }
    if (frame.type === "turn_report") {
      busyRef.current = false;
      setBusy(false);
      setLastTurn(turnReportFromFrame(frame));
      const id = sessionRef.current;
      if (id) {
        agent.getSession(id).then((loaded) => {
          if (sessionRef.current === id) setInfo(loaded);
        }).catch(() => void 0);
      }
    }
    setItems((prev) => applyFrame(prev, frame));
  };
  const attach = (id) => {
    if (!id || sessionRef.current === id) return;
    subRef.current?.dispose();
    sessionRef.current = id;
    subRef.current = agent.onSessionEvent(id, handleFrame);
    chatStore.setCurrent(id);
    setItems([]);
    setPending([]);
    busyRef.current = false;
    setBusy(false);
    setError(null);
    setLastTurn(null);
    setGoal(null);
    setPlan(null);
    setTodos([]);
    agent.getPending(id).then((list4) => setPending(list4.map(pendingFromRest))).catch(() => void 0);
    agent.getSession(id).then((loaded) => {
      if (sessionRef.current !== id) return;
      setInfo(loaded);
      setLastTurn(loaded.lastTurn ?? null);
    }).catch(() => void 0);
    agent.getGoal(id).then((g) => {
      if (sessionRef.current === id) setGoal(g);
    }).catch(() => void 0);
    agent.getPlan(id).then((p) => {
      if (sessionRef.current === id) setPlan(p);
    }).catch(() => void 0);
    agent.getTodos(id).then((list4) => {
      if (sessionRef.current === id) setTodos(list4);
    }).catch(() => void 0);
  };
  const refreshSessions = async () => {
    try {
      chatStore.setSessions(await agent.listSessions());
    } catch {
    }
  };
  const homeDirRef = useRef5(null);
  const defaultWorkspace = async () => {
    if (homeDirRef.current) return homeDirRef.current;
    try {
      const listing = await agent.listDirs();
      homeDirRef.current = listing.path;
      return listing.path;
    } catch {
      return void 0;
    }
  };
  const ensure = async () => {
    const id = sessionRef.current;
    if (id) return id;
    const workspace = draftCwd ?? await defaultWorkspace();
    const created = await agent.createSession(workspace, mode === "creator" ? "creator" : void 0);
    setMode("standard");
    setDraftCwd(void 0);
    attach(created.id);
    if (draftModel && created.model !== draftModel) {
      try {
        setInfo(await agent.setSessionModel(created.id, draftModel));
      } catch {
      }
    }
    return created.id;
  };
  useEffect6(() => {
    let alive = true;
    (async () => {
      try {
        const raw = await agent.listSessions();
        if (!alive) return;
        const list4 = chatStore.setSessions(raw);
        if (list4.length > 0) attach(list4[0].id);
      } catch (e) {
        if (alive) setError(`\u4F1A\u8BDD\u5217\u8868\u4E0D\u53EF\u8FBE\uFF1A${errText(e)}`);
      }
    })();
    return () => {
      alive = false;
      subRef.current?.dispose();
    };
  }, [agent]);
  useEffect6(() => {
    let alive = true;
    agent.listModels().then((data) => {
      if (!alive) return;
      setCatalog(data.providers);
      setDraftModel((prev) => prev || data.defaultModel);
    }).catch(() => void 0);
    storage.get("recentWorkspaces").then((list4) => {
      if (alive && Array.isArray(list4)) chatStore.setRecents(list4);
    }).catch(() => void 0);
    storage.get("lastModel").then((model) => {
      if (alive && typeof model === "string") setDraftModel(model);
    }).catch(() => void 0);
    return () => {
      alive = false;
    };
  }, [agent, storage]);
  const stopTurn = async () => {
    const id = sessionRef.current;
    if (!id) return;
    try {
      await agent.interrupt(id);
    } catch (e) {
      setError(`\u4E2D\u65AD\u5931\u8D25\uFF1A${errText(e)}`);
      busyRef.current = false;
      setBusy(false);
    }
  };
  const send = async () => {
    const text7 = input2.trim();
    const files = attachments;
    if (!text7 && files.length === 0) return;
    if (text7.startsWith("/") && files.length === 0) {
      await runSlash(text7);
      return;
    }
    if (busyRef.current) return;
    busyRef.current = true;
    setError(null);
    setInput("");
    setAttachments([]);
    setItems((prev) => [
      ...prev,
      { kind: "user", text: text7, attachments: files.length > 0 ? files : void 0, optimistic: true }
    ]);
    try {
      const id = await ensure();
      setBusy(true);
      await agent.send(id, text7, files);
      void refreshSessions();
    } catch (e) {
      busyRef.current = false;
      setBusy(false);
      setItems((prev) => {
        const last = prev[prev.length - 1];
        if (last?.kind === "user" && last.optimistic && last.text === text7 && (last.attachments?.length ?? 0) === files.length) {
          return prev.slice(0, -1);
        }
        return prev;
      });
      if (files.length > 0) setAttachments(files);
      setError(`\u53D1\u9001\u5931\u8D25\uFF1A${errText(e)}`);
    }
  };
  const newSession = async (cwd2) => {
    subRef.current?.dispose();
    subRef.current = null;
    sessionRef.current = null;
    chatStore.setCurrent(null);
    setItems([]);
    setPending([]);
    setAttachments([]);
    busyRef.current = false;
    setBusy(false);
    setError(null);
    setInfo(null);
    setLastTurn(null);
    setGoal(null);
    setPlan(null);
    setTodos([]);
    setDraftCwd(cwd2);
    if (cwd2) {
      const next = [cwd2, ...workspaces.filter((w) => w !== cwd2)].slice(0, 5);
      chatStore.setRecents(next);
      await storage.set("recentWorkspaces", next).catch(() => void 0);
    }
  };
  const lastCommandSeq = useRef5(0);
  useEffect6(() => {
    if (!command || command.seq === lastCommandSeq.current) return;
    lastCommandSeq.current = command.seq;
    if (command.kind === "select") attach(command.id);
    else void newSession(command.cwd);
  }, [command]);
  const ok3 = (text7) => ({ kind: "success", text: text7 });
  const fail = (text7) => ({ kind: "error", text: text7 });
  const runSlash = async (raw) => {
    const m = /^\/(\S+)(?:\s+([\s\S]*))?$/.exec(raw.trim());
    if (!m) return;
    const typed = m[1].toLowerCase();
    const name2 = COMMAND_ALIASES[typed] ?? typed;
    const arg = (m[2] ?? "").trim();
    const id = sessionRef.current;
    setInput("");
    if (name2 === "goal" || name2 === "plan" || name2 === "compact" || name2 === "feedback" || name2 === "permission") {
      if (!id) {
        setItems((prev) => [
          ...prev,
          { kind: "notice", tone: "error", text: `\u65E0\u4F1A\u8BDD\uFF0C\u65E0\u6CD5\u6267\u884C /${name2}` }
        ]);
        return;
      }
      try {
        await agent.runCommand(id, name2, arg);
      } catch (e) {
        setError(`/${name2} \u6267\u884C\u5931\u8D25\uFF1A${errText(e)}`);
      }
      return;
    }
    const itemId = `cmd-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    setItems((prev) => [
      ...prev,
      { kind: "command", id: itemId, name: name2, args: arg ? " " + arg : "", outcome: null }
    ]);
    const settle = (outcome) => setItems(
      (prev) => prev.map((it) => it.kind === "command" && it.id === itemId ? { ...it, outcome } : it)
    );
    try {
      switch (name2) {
        case "help":
          settle(ok3(HELP_TEXT));
          return;
        case "new":
          await newSession(arg || void 0);
          settle(ok3(arg ? `\u5DF2\u5207\u5230\u65B0\u4F1A\u8BDD\u8349\u7A3F\uFF08\u5DE5\u4F5C\u533A ${arg}\uFF09` : "\u5DF2\u5F00\u59CB\u65B0\u4F1A\u8BDD\uFF08\u53D1\u9001\u9996\u6761\u6D88\u606F\u540E\u521B\u5EFA\uFF09"));
          return;
        case "stop":
          if (!id) {
            settle(fail("\u65E0\u4F1A\u8BDD\u53EF\u4E2D\u65AD"));
            return;
          }
          await agent.interrupt(id);
          settle(ok3("\u5DF2\u8BF7\u6C42\u4E2D\u65AD\u5F53\u524D\u8F6E\u6B21"));
          return;
        case "model": {
          if (!id) {
            settle(fail("\u65E0\u4F1A\u8BDD\u53EF\u5207\u6362\u6A21\u578B"));
            return;
          }
          if (!arg) {
            settle(ok3(`\u5F53\u524D\u6A21\u578B\uFF1A${info?.model ?? "\u9ED8\u8BA4"}`));
            return;
          }
          setInfo(await agent.setSessionModel(id, arg));
          await storage.set("lastModel", arg);
          settle(ok3(`\u6A21\u578B\u5DF2\u5207\u6362\u4E3A ${arg}`));
          return;
        }
        case "approval": {
          if (!id) {
            settle(fail("\u65E0\u4F1A\u8BDD\u53EF\u5207\u6362\u5BA1\u6279\u9884\u8BBE"));
            return;
          }
          if (!APPROVAL_PRESETS.includes(arg)) {
            settle(fail(`\u672A\u77E5\u5BA1\u6279\u9884\u8BBE\uFF1A${arg || "\uFF08\u7A7A\uFF09"}\uFF08\u53EF\u7528\uFF1A${APPROVAL_PRESETS.join(" / ")}\uFF09`));
            return;
          }
          setInfo(await agent.setApprovalPreset(id, arg));
          settle(ok3(`\u5BA1\u6279\u9884\u8BBE\u5DF2\u5207\u6362\u4E3A ${arg}`));
          return;
        }
        case "export": {
          if (!id) {
            settle(fail("\u65E0\u4F1A\u8BDD\u53EF\u5BFC\u51FA"));
            return;
          }
          if (arg) {
            settle(fail("/export \u4E0D\u63A5\u53D7\u8DEF\u5F84\u53C2\u6570"));
            return;
          }
          const session = chatStore.snapshot().sessions.find((item) => item.id === id);
          if (!session) {
            settle(fail("\u4F1A\u8BDD\u4FE1\u606F\u5C1A\u672A\u52A0\u8F7D\uFF0C\u7A0D\u540E\u91CD\u8BD5"));
            return;
          }
          downloadText(sessionLabel(session), await sessionToMarkdown(agent, session));
          settle(ok3("\u5DF2\u5BFC\u51FA\u4F1A\u8BDD Markdown"));
          return;
        }
        default:
          settle(fail(`\u672A\u77E5\u547D\u4EE4 /${name2}\uFF08\u8F93\u5165 /help \u67E5\u770B\u547D\u4EE4\u5217\u8868\uFF09`));
      }
    } catch (e) {
      settle(fail(`/${name2} \u6267\u884C\u5931\u8D25\uFF1A${errText(e)}`));
    }
  };
  const onModelChange = async (model) => {
    if (!model) return;
    const id = sessionRef.current;
    if (!id) {
      setDraftModel(model);
      await storage.set("lastModel", model).catch(() => void 0);
      return;
    }
    try {
      setInfo(await agent.setSessionModel(id, model));
      await storage.set("lastModel", model);
      setDraftModel(model);
    } catch (e) {
      setError(`\u6A21\u578B\u5207\u6362\u5931\u8D25\uFF1A${errText(e)}`);
    }
  };
  const onApprovalChange = async (preset) => {
    const id = sessionRef.current;
    if (!id) return;
    try {
      setInfo(await agent.setApprovalPreset(id, preset));
    } catch (e) {
      setError(`\u5BA1\u6279\u6A21\u5F0F\u5207\u6362\u5931\u8D25\uFF1A${errText(e)}`);
    }
  };
  const removePending = (requestId) => {
    setPending((prev) => prev.filter((p) => p.requestId !== requestId));
  };
  const onGoalAction = async (action) => {
    const id = sessionRef.current;
    if (!id) return;
    try {
      const next = await agent.goalAction(id, action, goal?.revision ?? 0);
      setGoal(next);
    } catch (e) {
      setError(`\u76EE\u6807\u64CD\u4F5C\u5931\u8D25\uFF1A${errText(e)}`);
    }
  };
  const onGoalEdit = async (objective) => {
    const id = sessionRef.current;
    if (!id) return;
    try {
      setGoal(await agent.setGoal(id, { objective, ...goal ? { revision: goal.revision } : {} }));
    } catch (e) {
      setError(`\u76EE\u6807\u66F4\u65B0\u5931\u8D25\uFF1A${errText(e)}`);
    }
  };
  const onExitPlan = async () => {
    const id = sessionRef.current;
    if (!id) return;
    try {
      setPlan(await agent.setPlan(id, false));
    } catch (e) {
      setError(`\u9000\u51FA plan \u6A21\u5F0F\u5931\u8D25\uFF1A${errText(e)}`);
    }
  };
  const onDecision = async (requestId, decision) => {
    const id = sessionRef.current;
    if (!id) return;
    try {
      await agent.replyApproval(id, requestId, decision);
      removePending(requestId);
    } catch (e) {
      setError(`\u5BA1\u6279\u56DE\u586B\u5931\u8D25\uFF1A${errText(e)}`);
    }
  };
  const onVerdict = async (requestId, verdict) => {
    const id = sessionRef.current;
    if (!id) return;
    try {
      await agent.replyPlanReview(id, requestId, verdict);
      removePending(requestId);
    } catch (e) {
      setError(`\u8BA1\u5212\u8BC4\u5BA1\u56DE\u586B\u5931\u8D25\uFF1A${errText(e)}`);
    }
  };
  const onSubmitAnswers = async (requestId, answers) => {
    const id = sessionRef.current;
    if (!id) return;
    try {
      await agent.answerQuestion(id, requestId, answers);
      removePending(requestId);
    } catch (e) {
      setError(`\u95EE\u8BE2\u7B54\u590D\u5931\u8D25\uFF1A${errText(e)}`);
    }
  };
  const slashMatch = /^\/(\S*)$/.exec(input2);
  const slashMenuOpen = slashMatch !== null && !busy;
  const slashQuery = (slashMatch?.[1] ?? "").toLowerCase();
  const slashFiltered = slashMenuOpen ? SLASH_COMMANDS.filter((c) => ("/" + c.name).startsWith("/" + slashQuery)) : [];
  const slashActive = Math.min(slashIndex, Math.max(slashFiltered.length - 1, 0));
  const [slashOverflowBelow, setSlashOverflowBelow] = useState7(false);
  useEffect6(() => {
    if (!slashMenuOpen || slashDismissed) return;
    const viewport = slashListRef.current;
    setSlashOverflowBelow(viewport !== null && viewport.scrollTop + viewport.clientHeight < viewport.scrollHeight - 1);
  });
  useEffect6(() => {
    if (!slashMenuOpen || slashDismissed || slashFiltered.length === 0) return;
    document.getElementById(`cc-slash-option-${slashActive}`)?.scrollIntoView({ block: "nearest" });
  }, [slashMenuOpen, slashDismissed, slashActive, slashFiltered.length]);
  const onKeyDown = (e) => {
    if (slashMenuOpen && !slashDismissed && slashFiltered.length > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSlashIndex((i) => (i + 1) % slashFiltered.length);
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSlashIndex((i) => (i - 1 + slashFiltered.length) % slashFiltered.length);
        return;
      }
      if (e.key === "Tab" || e.key === "Enter") {
        e.preventDefault();
        const cmd = slashFiltered[slashActive];
        if (cmd.args && !(e.key === "Enter" && input2.trim() === "/" + cmd.name)) {
          setInput("/" + cmd.name + " ");
          setSlashDismissed(true);
        } else {
          void send();
        }
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        setSlashDismissed(true);
        return;
      }
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void send();
    }
  };
  const canSend = !busy && (input2.trim() !== "" || attachments.length > 0);
  return /* @__PURE__ */ jsxs6("div", { className: "cc-root", children: [
    /* @__PURE__ */ jsx7("style", { children: chatappCss }),
    /* @__PURE__ */ jsxs6("header", { className: "cc-head", children: [
      /* @__PURE__ */ jsxs6("div", { className: "cc-viewtabs", role: "tablist", "aria-label": "\u4E2D\u680F\u89C6\u56FE", children: [
        /* @__PURE__ */ jsx7(
          "button",
          {
            className: "cc-viewtab" + (view === "chat" ? " on" : ""),
            role: "tab",
            "aria-selected": view === "chat",
            onClick: () => setView("chat"),
            children: "\u5BF9\u8BDD"
          }
        ),
        /* @__PURE__ */ jsx7(
          "button",
          {
            className: "cc-viewtab" + (view === "trajectory" ? " on" : ""),
            role: "tab",
            "aria-selected": view === "trajectory",
            onClick: () => setView("trajectory"),
            children: "\u8F68\u8FF9"
          }
        )
      ] }),
      /* @__PURE__ */ jsxs6("div", { className: "cc-head-right", children: [
        busy && /* @__PURE__ */ jsx7("span", { className: "cc-head-dim", children: "\u751F\u6210\u4E2D\u2026" }),
        /* @__PURE__ */ jsx7(
          ContextMeter,
          {
            tokens: info?.contextTokens ?? 0,
            limit: info?.contextWindowTokens ?? 0
          }
        )
      ] })
    ] }),
    /* @__PURE__ */ jsx7("div", { className: "cc-main", children: view === "trajectory" ? /* @__PURE__ */ jsx7(TrajectoryView, { items }) : /* @__PURE__ */ jsx7("div", { className: "cc-scroll", ref: scrollRef, children: items.length === 0 && pending.length === 0 ? /* @__PURE__ */ jsxs6("div", { className: "cc-empty", children: [
      /* @__PURE__ */ jsx7("span", { className: "cc-empty-ico", children: /* @__PURE__ */ jsx7(
        "svg",
        {
          width: "20",
          height: "20",
          viewBox: "0 0 24 24",
          fill: "none",
          stroke: "currentColor",
          strokeWidth: "1.8",
          strokeLinecap: "round",
          strokeLinejoin: "round",
          children: /* @__PURE__ */ jsx7("path", { d: "M7.9 20A9 9 0 1 0 4 16.1L2 22Z" })
        }
      ) }),
      /* @__PURE__ */ jsx7("span", { className: "cc-empty-title", children: "\u5F00\u59CB\u65B0\u7684\u5BF9\u8BDD" }),
      /* @__PURE__ */ jsx7("span", { children: "\u5728\u4E0B\u65B9\u8F93\u5165\u6D88\u606F\uFF0CAgent \u4F1A\u5373\u65F6\u54CD\u5E94\u3002" })
    ] }) : /* @__PURE__ */ jsxs6("div", { className: "cc-flow", children: [
      /* @__PURE__ */ jsx7(ChatItems, { items, sessionId: sessionId ?? void 0 }),
      pending.map(
        (card) => card.kind === "approval" ? /* @__PURE__ */ jsx7(
          ApprovalCardView,
          {
            card,
            onDecision: (id, d) => void onDecision(id, d)
          },
          card.requestId
        ) : card.kind === "plan_review" ? /* @__PURE__ */ jsx7(
          PlanReviewCardView,
          {
            card,
            onVerdict: (id, v) => void onVerdict(id, v)
          },
          card.requestId
        ) : /* @__PURE__ */ jsx7(
          QuestionCardView,
          {
            card,
            onSubmit: (id, a) => void onSubmitAnswers(id, a)
          },
          card.requestId
        )
      )
    ] }) }) }),
    error && /* @__PURE__ */ jsx7("div", { className: "cc-error", children: error }),
    view === "chat" && /* @__PURE__ */ jsxs6("div", { className: "cc-composer-wrap", children: [
      /* @__PURE__ */ jsx7(TodoPanel, { todos }),
      goal && goal.phase !== "complete" && /* @__PURE__ */ jsx7(GoalBar, { goal, onAction: (action) => void onGoalAction(action), onEdit: (o) => void onGoalEdit(o) }),
      /* @__PURE__ */ jsxs6("div", { className: "cc-composer", children: [
        slashMenuOpen && !slashDismissed && slashFiltered.length > 0 && /* @__PURE__ */ jsx7("div", { className: "cc-slash" + (slashOverflowBelow ? " is-overflow-below" : ""), children: /* @__PURE__ */ jsx7(
          "div",
          {
            ref: slashListRef,
            className: "cc-slash-list",
            role: "listbox",
            "aria-label": "\u547D\u4EE4\u5EFA\u8BAE",
            "aria-activedescendant": `cc-slash-option-${slashActive}`,
            onScroll: () => {
              const vp = slashListRef.current;
              if (vp) setSlashOverflowBelow(vp.scrollTop + vp.clientHeight < vp.scrollHeight - 1);
            },
            children: slashFiltered.map((c, i) => {
              const prev = slashFiltered[i - 1];
              return /* @__PURE__ */ jsxs6(Fragment6, { children: [
                !prev || prev.section !== c.section ? /* @__PURE__ */ jsx7("div", { className: "cc-slash-section", children: SECTION_LABELS[c.section] }) : null,
                /* @__PURE__ */ jsxs6(
                  "button",
                  {
                    id: `cc-slash-option-${i}`,
                    type: "button",
                    role: "option",
                    "aria-selected": i === slashActive,
                    className: "cc-slash-item" + (i === slashActive ? " on" : ""),
                    onMouseDown: (e) => {
                      e.preventDefault();
                      setSlashIndex(i);
                      if (c.args) {
                        setInput("/" + c.name + " ");
                        setSlashDismissed(true);
                      } else {
                        void send();
                      }
                    },
                    onMouseMove: i === slashActive ? void 0 : () => setSlashIndex(i),
                    children: [
                      /* @__PURE__ */ jsx7("span", { className: "cc-slash-ico", "aria-hidden": true, children: /* @__PURE__ */ jsx7(Icon, { d: c.icon, size: 14 }) }),
                      /* @__PURE__ */ jsx7("span", { className: "cc-slash-label", children: c.label }),
                      c.label.toLowerCase() !== c.name.toLowerCase() && /* @__PURE__ */ jsx7("span", { className: "cc-slash-name", children: c.name }),
                      /* @__PURE__ */ jsx7("span", { className: "cc-slash-desc", children: c.desc })
                    ]
                  }
                )
              ] }, c.name);
            })
          }
        ) }),
        attachments.length > 0 && /* @__PURE__ */ jsx7("div", { className: "cc-attach-list", children: attachments.map((a, i) => /* @__PURE__ */ jsxs6("div", { className: "cc-attach-chip", title: `${a.name} \xB7 ${a.mimeType}`, children: [
          /* @__PURE__ */ jsx7("img", { className: "cc-attach-thumb", src: a.dataUrl, alt: a.name || "\u56FE\u7247" }),
          /* @__PURE__ */ jsx7("span", { className: "cc-attach-name", children: a.name || "\u56FE\u7247" }),
          /* @__PURE__ */ jsx7(
            "button",
            {
              className: "cc-attach-x",
              onClick: () => setAttachments((prev) => prev.filter((_, j) => j !== i)),
              "aria-label": `\u79FB\u9664 ${a.name || "\u56FE\u7247"}`,
              title: "\u79FB\u9664",
              children: /* @__PURE__ */ jsx7(Icon, { d: ICON2.close, size: 12 })
            }
          )
        ] }, `${a.name}-${i}`)) }),
        /* @__PURE__ */ jsx7(
          "textarea",
          {
            className: "cc-input",
            ref: inputRef,
            value: input2,
            onChange: (e) => {
              setInput(e.target.value);
              setSlashIndex(0);
              setSlashDismissed(false);
            },
            onKeyDown,
            onPaste: (e) => {
              const files = Array.from(e.clipboardData?.files ?? []);
              if (files.length > 0) {
                e.preventDefault();
                void addFiles(files);
              }
            },
            placeholder: "\u8F93\u5165\u6D88\u606F\uFF0C/ \u547C\u51FA\u547D\u4EE4",
            rows: 1
          }
        ),
        /* @__PURE__ */ jsxs6("div", { className: "cc-bar", children: [
          /* @__PURE__ */ jsxs6("div", { className: "cc-bar-left", children: [
            /* @__PURE__ */ jsx7(
              "input",
              {
                ref: fileRef,
                type: "file",
                accept: "image/*",
                multiple: true,
                hidden: true,
                onChange: (e) => {
                  const files = e.target.files;
                  if (files && files.length > 0) void addFiles(files);
                  e.target.value = "";
                }
              }
            ),
            /* @__PURE__ */ jsx7(
              "button",
              {
                className: "cc-attach-btn",
                onClick: () => fileRef.current?.click(),
                title: "\u6DFB\u52A0\u56FE\u7247\u9644\u4EF6",
                "aria-label": "\u6DFB\u52A0\u56FE\u7247\u9644\u4EF6",
                children: /* @__PURE__ */ jsx7(Icon, { d: ICON2.attach, size: 14 })
              }
            ),
            /* @__PURE__ */ jsx7(
              WorkspaceChip,
              {
                current: info?.workingDir || info?.root || draftCwd,
                workspaces,
                onPick: (dir) => void newSession(dir),
                onBrowse: () => setWsPicker(true)
              }
            ),
            /* @__PURE__ */ jsx7(
              ApprovalChip,
              {
                value: info?.approvalPreset ?? "workspace-write",
                disabled: !sessionId,
                onChange: (preset) => void onApprovalChange(preset)
              }
            ),
            plan && /* @__PURE__ */ jsx7(PlanChip, { plan, onExit: () => void onExitPlan() }),
            /* @__PURE__ */ jsx7(ModeChip, { value: mode, onChange: setMode })
          ] }),
          /* @__PURE__ */ jsxs6("div", { className: "cc-bar-right", children: [
            /* @__PURE__ */ jsx7(
              ModelChip,
              {
                value: info?.model ?? draftModel,
                catalog,
                disabled: catalog.length === 0,
                onChange: (model) => void onModelChange(model)
              }
            ),
            /* @__PURE__ */ jsx7(
              "button",
              {
                className: "cc-send" + (busy ? " is-stop" : ""),
                onClick: () => busy ? void stopTurn() : void send(),
                disabled: !busy && !canSend,
                title: busy ? "\u505C\u6B62\u751F\u6210" : "\u53D1\u9001",
                "aria-label": busy ? "\u505C\u6B62\u751F\u6210" : "\u53D1\u9001",
                children: /* @__PURE__ */ jsx7(
                  "svg",
                  {
                    width: "16",
                    height: "16",
                    viewBox: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    strokeWidth: "2",
                    strokeLinecap: "round",
                    strokeLinejoin: "round",
                    "aria-hidden": "true",
                    children: busy ? /* @__PURE__ */ jsx7("rect", { x: "7", y: "7", width: "10", height: "10", rx: "1.5", fill: "currentColor", stroke: "none" }) : /* @__PURE__ */ jsx7("path", { d: "M12 19V5 M5 12l7-7 7 7" })
                  }
                )
              }
            )
          ] })
        ] })
      ] })
    ] }),
    wsPicker && /* @__PURE__ */ jsx7(
      WorkspacePicker,
      {
        agent,
        startDir: info?.workingDir || info?.root || draftCwd,
        recent: workspaces,
        native: pickNative,
        onCancel: () => setWsPicker(false),
        onPick: (dir) => {
          setWsPicker(false);
          void newSession(dir);
        }
      }
    )
  ] });
}

// src/session-sidebar.tsx
import { useEffect as useEffect7, useLayoutEffect as useLayoutEffect2, useRef as useRef6, useState as useState8 } from "react";
import { jsx as jsx8, jsxs as jsxs7 } from "react/jsx-runtime";
var SESSION_LIMIT = 5;
function Popconfirm({
  title,
  onConfirm,
  onClose
}) {
  const ref = useRef6(null);
  const [pos, setPos] = useState8(null);
  useEffect7(() => {
    const onDoc = (e) => {
      if (ref.current && !ref.current.contains(e.target)) onClose();
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [onClose]);
  useLayoutEffect2(() => {
    const el = ref.current;
    const anchor = el?.parentElement;
    if (!el || !anchor) return;
    const r = anchor.getBoundingClientRect();
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    const left = Math.max(8, Math.min(r.right - w, window.innerWidth - w - 8));
    let top = r.bottom + 6;
    if (top + h > window.innerHeight - 8) top = Math.max(8, r.top - h - 6);
    setPos({ top, left });
  }, []);
  return /* @__PURE__ */ jsxs7(
    "div",
    {
      className: "cc-pc",
      ref,
      role: "dialog",
      "aria-label": "\u786E\u8BA4\u64CD\u4F5C",
      style: pos ? { top: pos.top, left: pos.left } : { visibility: "hidden" },
      onClick: (e) => e.stopPropagation(),
      children: [
        /* @__PURE__ */ jsx8("div", { className: "cc-pc-msg", children: title }),
        /* @__PURE__ */ jsxs7("div", { className: "cc-pc-acts", children: [
          /* @__PURE__ */ jsx8("button", { className: "cc-pc-cancel", onClick: onClose, children: "\u53D6\u6D88" }),
          /* @__PURE__ */ jsx8(
            "button",
            {
              className: "cc-pc-confirm",
              onClick: () => {
                onClose();
                onConfirm();
              },
              children: "\u786E\u8BA4"
            }
          )
        ] })
      ]
    }
  );
}
function FolderIcon2({ open, current }) {
  return open || current ? /* @__PURE__ */ jsx8("svg", { width: "16", height: "16", viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: "1.8", strokeLinecap: "round", strokeLinejoin: "round", "aria-hidden": "true", children: /* @__PURE__ */ jsx8("path", { d: "M3 7.5A1.5 1.5 0 0 1 4.5 6h5l2 2h8A1.5 1.5 0 0 1 21 9.5v8A1.5 1.5 0 0 1 19.5 19h-15A1.5 1.5 0 0 1 3 17.5v-10Z" }) }) : /* @__PURE__ */ jsx8("svg", { width: "16", height: "16", viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: "1.8", strokeLinecap: "round", strokeLinejoin: "round", "aria-hidden": "true", children: /* @__PURE__ */ jsx8("path", { d: "M3 7.5A1.5 1.5 0 0 1 4.5 6h5l2 2h8A1.5 1.5 0 0 1 21 9.5v8A1.5 1.5 0 0 1 19.5 19h-15A1.5 1.5 0 0 1 3 17.5v-10Z" }) });
}
function ChevronIcon({ open }) {
  return /* @__PURE__ */ jsx8(
    "svg",
    {
      className: "cc-sb-chev" + (open ? " open" : ""),
      width: "10",
      height: "10",
      viewBox: "0 0 24 24",
      fill: "none",
      stroke: "currentColor",
      strokeWidth: "2.5",
      strokeLinecap: "round",
      strokeLinejoin: "round",
      "aria-hidden": "true",
      children: /* @__PURE__ */ jsx8("path", { d: "M9 6l6 6-6 6" })
    }
  );
}
function SessionRow({
  session,
  active,
  busy,
  onExport,
  onRemove
}) {
  const [confirmOpen, setConfirmOpen] = useState8(false);
  return /* @__PURE__ */ jsxs7("div", { className: "cc-sb-item", children: [
    /* @__PURE__ */ jsxs7(
      "button",
      {
        className: "cc-sb-row" + (active ? " on" : ""),
        "data-session-id": session.id,
        onClick: () => chatStore.select(session.id),
        title: sessionLabel(session),
        "aria-current": active ? "true" : void 0,
        children: [
          /* @__PURE__ */ jsx8("span", { className: "cc-sb-row-slot", children: session.live && /* @__PURE__ */ jsx8("span", { className: "cc-sb-live" }) }),
          /* @__PURE__ */ jsx8("span", { className: "cc-sb-row-title", children: sessionLabel(session) }),
          /* @__PURE__ */ jsx8("span", { className: "cc-sb-row-time", children: sessionTime(session.updatedAtMs) })
        ]
      }
    ),
    /* @__PURE__ */ jsxs7("span", { className: "cc-sb-acts", children: [
      /* @__PURE__ */ jsx8(
        "button",
        {
          className: "cc-sb-act",
          title: "\u5BFC\u51FA\u4E3A Markdown",
          disabled: busy,
          onClick: () => onExport(session),
          children: /* @__PURE__ */ jsx8("svg", { width: "13", height: "13", viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: "1.8", strokeLinecap: "round", strokeLinejoin: "round", "aria-hidden": "true", children: /* @__PURE__ */ jsx8("path", { d: "M12 4v11 M7 11l5 5 5-5 M4 20h16" }) })
        }
      ),
      /* @__PURE__ */ jsx8(
        "button",
        {
          className: "cc-sb-act is-danger",
          title: "\u5220\u9664\u4F1A\u8BDD",
          disabled: busy,
          onClick: () => setConfirmOpen((v) => !v),
          children: /* @__PURE__ */ jsx8("svg", { width: "13", height: "13", viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: "1.8", strokeLinecap: "round", strokeLinejoin: "round", "aria-hidden": "true", children: /* @__PURE__ */ jsx8("path", { d: "M4 7h16 M9 7V5h6v2 M6 7l1 13h10l1-13" }) })
        }
      )
    ] }),
    confirmOpen && /* @__PURE__ */ jsx8("span", { className: "cc-pc-anchor", children: /* @__PURE__ */ jsx8(
      Popconfirm,
      {
        title: `\u5220\u9664\u4F1A\u8BDD\u300C${sessionLabel(session)}\u300D\uFF1F\u5386\u53F2\u8BB0\u5F55\u5C06\u4ECE\u78C1\u76D8\u79FB\u9664\uFF0C\u4E0D\u53EF\u6062\u590D\u3002`,
        onClose: () => setConfirmOpen(false),
        onConfirm: () => void onRemove(session)
      }
    ) })
  ] });
}
function SessionSidebar({ agent }) {
  const { sessions, currentId } = useChatStore();
  const groups = groupSessions(sessions);
  const currentKey = sessions.find((s) => s.id === currentId)?.workingDir;
  const lastKeyRef = useRef6(currentKey);
  if (currentKey !== void 0) lastKeyRef.current = currentKey;
  const anchorKey = currentKey ?? lastKeyRef.current;
  const [expanded, setExpanded] = useState8(/* @__PURE__ */ new Set());
  const [collapsed, setCollapsed] = useState8(/* @__PURE__ */ new Set());
  const [revealed, setRevealed] = useState8(/* @__PURE__ */ new Set());
  const [busyId, setBusyId] = useState8(null);
  const [busyWs, setBusyWs] = useState8(null);
  const [error, setError] = useState8(null);
  const toggle = (key, isCurrent) => {
    if (isCurrent) {
      setCollapsed((prev) => {
        const next = new Set(prev);
        if (next.has(key)) next.delete(key);
        else next.add(key);
        return next;
      });
    } else {
      setExpanded((prev) => {
        const next = new Set(prev);
        if (next.has(key)) next.delete(key);
        else next.add(key);
        return next;
      });
    }
  };
  const refresh = async () => {
    try {
      chatStore.setSessions(await agent.listSessions());
    } catch (e) {
      setError(`\u4F1A\u8BDD\u5217\u8868\u5237\u65B0\u5931\u8D25\uFF1A${e instanceof Error ? e.message : String(e)}`);
    }
  };
  const [pendingWs, setPendingWs] = useState8(null);
  const remove = async (session) => {
    setBusyId(session.id);
    setError(null);
    try {
      await agent.deleteSession(session.id);
      const rest = sessions.filter((s) => s.id !== session.id);
      chatStore.setSessions(rest);
      if (currentId === session.id) {
        if (rest.length > 0) chatStore.select(rest[0].id);
        else chatStore.create(session.workingDir);
      }
      await refresh();
    } catch (e) {
      setError(`\u5220\u9664\u4F1A\u8BDD\u5931\u8D25\uFF1A${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setBusyId(null);
    }
  };
  const removeWorkspace = async (group) => {
    setBusyWs(group.key);
    setError(null);
    try {
      for (const s of group.sessions) {
        await agent.deleteSession(s.id);
      }
      const rest = sessions.filter((s) => (s.workingDir ?? "") !== group.key);
      chatStore.setSessions(rest);
      if (currentId !== null && group.sessions.some((s) => s.id === currentId)) {
        if (rest.length > 0) chatStore.select(rest[0].id);
        else chatStore.create(group.key || void 0);
      }
      await refresh();
    } catch (e) {
      setError(`\u5220\u9664\u5DE5\u4F5C\u533A\u5931\u8D25\uFF1A${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setBusyWs(null);
    }
  };
  const exportOne = async (session) => {
    setBusyId(session.id);
    setError(null);
    try {
      downloadText(sessionLabel(session), await sessionToMarkdown(agent, session));
    } catch (e) {
      setError(`\u5BFC\u51FA\u4F1A\u8BDD\u5931\u8D25\uFF1A${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setBusyId(null);
    }
  };
  return /* @__PURE__ */ jsxs7("div", { className: "cc-root cc-sb", children: [
    /* @__PURE__ */ jsx8("style", { children: chatappCss }),
    /* @__PURE__ */ jsxs7("button", { className: "cc-sb-new", onClick: () => chatStore.create(), title: "\u65B0\u5EFA\u4F1A\u8BDD", children: [
      /* @__PURE__ */ jsx8("svg", { width: "14", height: "14", viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: "2", strokeLinecap: "round", "aria-hidden": "true", children: /* @__PURE__ */ jsx8("path", { d: "M12 5v14M5 12h14" }) }),
      "\u65B0\u5EFA\u4F1A\u8BDD"
    ] }),
    error && /* @__PURE__ */ jsx8("div", { className: "cc-sb-error", children: error }),
    /* @__PURE__ */ jsxs7("div", { className: "cc-sb-list", children: [
      groups.length === 0 && /* @__PURE__ */ jsx8("div", { className: "cc-sb-empty", children: "\u6682\u65E0\u5386\u53F2\u4F1A\u8BDD" }),
      groups.map((group) => {
        const isCurrent = group.sessions.some((s) => s.id === currentId);
        const anchored = group.key === anchorKey;
        const open = anchored ? !collapsed.has(group.key) : expanded.has(group.key);
        const showAll = revealed.has(group.key);
        const visible = showAll ? group.sessions : group.sessions.slice(0, SESSION_LIMIT);
        const hidden = group.sessions.length - visible.length;
        return /* @__PURE__ */ jsxs7("div", { className: "cc-sb-group" + (isCurrent ? " is-current" : ""), children: [
          /* @__PURE__ */ jsxs7(
            "div",
            {
              className: "cc-sb-group-head",
              role: "button",
              tabIndex: 0,
              onClick: () => toggle(group.key, anchored),
              onKeyDown: (e) => {
                if (e.key === "Enter" || e.key === " ") toggle(group.key, anchored);
              },
              title: group.key || "\u672A\u5F52\u7C7B\u4F1A\u8BDD",
              children: [
                /* @__PURE__ */ jsx8(ChevronIcon, { open }),
                /* @__PURE__ */ jsx8("span", { className: "cc-sb-folder", children: /* @__PURE__ */ jsx8(FolderIcon2, { open, current: isCurrent }) }),
                /* @__PURE__ */ jsx8("span", { className: "cc-sb-group-label", children: group.label }),
                /* @__PURE__ */ jsx8(
                  "button",
                  {
                    className: "cc-sb-del",
                    title: `\u5220\u9664\u5DE5\u4F5C\u533A\u300C${group.label}\u300D\u53CA\u5176\u5168\u90E8 ${group.sessions.length} \u4E2A\u4F1A\u8BDD`,
                    disabled: busyWs !== null,
                    onClick: (e) => {
                      e.stopPropagation();
                      setPendingWs(pendingWs === group.key ? null : group.key);
                    },
                    children: /* @__PURE__ */ jsx8("svg", { width: "12", height: "12", viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: "2", strokeLinecap: "round", strokeLinejoin: "round", "aria-hidden": "true", children: /* @__PURE__ */ jsx8("path", { d: "M4 7h16 M9 7V5h6v2 M6 7l1 13h10l1-13" }) })
                  }
                ),
                pendingWs === group.key && /* @__PURE__ */ jsx8("span", { className: "cc-pc-anchor head", children: /* @__PURE__ */ jsx8(
                  Popconfirm,
                  {
                    title: `\u5220\u9664\u5DE5\u4F5C\u533A\u300C${group.label}\u300D\u53CA\u5176\u5168\u90E8 ${group.sessions.length} \u4E2A\u4F1A\u8BDD\uFF1F\u5386\u53F2\u8BB0\u5F55\u5C06\u4ECE\u78C1\u76D8\u79FB\u9664\uFF0C\u4E0D\u53EF\u6062\u590D\u3002`,
                    onClose: () => setPendingWs(null),
                    onConfirm: () => void removeWorkspace(group)
                  }
                ) }),
                /* @__PURE__ */ jsx8(
                  "button",
                  {
                    className: "cc-sb-add",
                    title: `\u5728 ${group.label} \u65B0\u5EFA\u4F1A\u8BDD`,
                    onClick: (e) => {
                      e.stopPropagation();
                      chatStore.create(group.key || void 0);
                    },
                    children: /* @__PURE__ */ jsx8("svg", { width: "12", height: "12", viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: "2", strokeLinecap: "round", "aria-hidden": "true", children: /* @__PURE__ */ jsx8("path", { d: "M12 5v14M5 12h14" }) })
                  }
                )
              ]
            }
          ),
          open && visible.map((session) => /* @__PURE__ */ jsx8(
            SessionRow,
            {
              session,
              active: session.id === currentId,
              busy: busyId === session.id,
              onExport: (s) => void exportOne(s),
              onRemove: (s) => void remove(s)
            },
            session.id
          )),
          open && hidden > 0 && /* @__PURE__ */ jsxs7(
            "button",
            {
              className: "cc-sb-more",
              onClick: () => setRevealed((prev) => {
                const next = new Set(prev);
                next.add(group.key);
                return next;
              }),
              children: [
                "\u5C55\u5F00\u5176\u4F59 ",
                hidden,
                " \u4E2A\u4F1A\u8BDD"
              ]
            }
          ),
          open && showAll && group.sessions.length > SESSION_LIMIT && /* @__PURE__ */ jsx8(
            "button",
            {
              className: "cc-sb-more",
              onClick: () => setRevealed((prev) => {
                const next = new Set(prev);
                next.delete(group.key);
                return next;
              }),
              children: "\u6536\u8D77"
            }
          )
        ] }, group.key);
      })
    ] })
  ] });
}

// src/main.tsx
import { jsx as jsx10 } from "react/jsx-runtime";
function setup(ctx) {
  setCanvasBus(ctx.events);
  ctx.registerSettingsPanel(() => Promise.resolve().then(() => (init_settings(), settings_exports)));
  const disposers = [
    ctx.slots.inject("nav.sidebar", {
      order: 5,
      viewId: "chatapp",
      title: "\u5BF9\u8BDD",
      // message-circle（lucide 同源）：单气泡对话，描边 currentColor
      icon: "M7.9 20A9 9 0 1 0 4 16.1L2 22z"
    }),
    // 会话历史（按工作区分组）挂在宿主侧边栏，仅本视图处于激活态时渲染
    ctx.slots.inject("panel.sidebar", {
      order: 5,
      viewId: "chatapp",
      title: "\u4F1A\u8BDD\u5386\u53F2",
      component: async () => ({ default: () => /* @__PURE__ */ jsx10(SessionSidebar, { agent: ctx.agent }) })
    }),
    ctx.slots.inject("surface.main", {
      viewId: "chatapp",
      title: "\u5BF9\u8BDD",
      component: async () => ({
        default: () => /* @__PURE__ */ jsx10(ChatView, { agent: ctx.agent, storage: ctx.storage, pickNative: ctx.desktop?.pickFolder })
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
