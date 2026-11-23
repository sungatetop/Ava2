var __defProp = Object.defineProperty;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __esm = (fn, res) => function __init() {
  return fn && (res = (0, fn[__getOwnPropNames(fn)[0]])(fn = 0)), res;
};
var __export = (target, all) => {
  for (var name in all)
    __defProp(target, name, { get: all[name], enumerable: true });
};

// src/config.ts
function readConfig() {
  try {
    return { ...DEFAULTS, ...JSON.parse(localStorage.getItem(CFG_KEY) || "{}") };
  } catch {
    return DEFAULTS;
  }
}
function writeConfig(patch) {
  localStorage.setItem(CFG_KEY, JSON.stringify({ ...readConfig(), ...patch }));
}
var CFG_KEY, DEFAULTS;
var init_config = __esm({
  "src/config.ts"() {
    "use strict";
    CFG_KEY = "task-manager:cfg";
    DEFAULTS = { confirmDelete: true };
  }
});

// src/settings-panel.tsx
var settings_panel_exports = {};
__export(settings_panel_exports, {
  default: () => SettingsPanel
});
import { useState as useState5 } from "react";
import { jsx as jsx6, jsxs as jsxs6 } from "react/jsx-runtime";
function SettingsPanel() {
  const [confirmDelete, setConfirmDelete] = useState5(() => readConfig().confirmDelete);
  return /* @__PURE__ */ jsxs6("div", { style: { display: "flex", flexDirection: "column", gap: 10, padding: "4px 0", fontSize: 13 }, children: [
    /* @__PURE__ */ jsxs6("label", { style: { display: "flex", alignItems: "center", gap: 8, cursor: "pointer" }, children: [
      /* @__PURE__ */ jsx6(
        "input",
        {
          type: "checkbox",
          checked: confirmDelete,
          onChange: (e) => {
            setConfirmDelete(e.target.checked);
            writeConfig({ confirmDelete: e.target.checked });
          }
        }
      ),
      "\u5220\u9664\u524D\u5F39\u51FA\u6C14\u6CE1\u786E\u8BA4"
    ] }),
    /* @__PURE__ */ jsx6("div", { style: { fontSize: 12, color: "var(--ava-label-tertiary)" }, children: "\u914D\u7F6E\u7531\u672C App \u81EA\u884C\u5B58\u50A8\uFF08localStorage\uFF09\uFF0C\u5373\u65F6\u751F\u6548\u3002" })
  ] });
}
var init_settings_panel = __esm({
  "src/settings-panel.tsx"() {
    "use strict";
    init_config();
  }
});

// src/tasks-view.tsx
init_config();
import { useCallback, useEffect as useEffect3, useState as useState4 } from "react";

// src/format.ts
var TZ = Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
function fmtWhen(rfc3339) {
  if (!rfc3339) return "\u2014";
  const date = new Date(rfc3339);
  if (Number.isNaN(date.getTime())) return rfc3339;
  const pad = (n) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}
function localToRfc3339(datetimeLocal) {
  const date = new Date(datetimeLocal);
  if (Number.isNaN(date.getTime())) return "";
  const tzo = -date.getTimezoneOffset();
  const sign = tzo >= 0 ? "+" : "-";
  const abs = Math.abs(tzo);
  const pad = (n) => String(n).padStart(2, "0");
  return `${datetimeLocal.length === 16 ? `${datetimeLocal}:00` : datetimeLocal}${sign}${pad(Math.floor(abs / 60))}:${pad(abs % 60)}`;
}
var KIND_LABEL = {
  at: "\u4E00\u6B21\u6027",
  after: "\u5EF6\u65F6",
  every: "\u95F4\u9694",
  daily: "\u6BCF\u5929",
  weekly: "\u6BCF\u5468",
  cron: "Cron"
};

// src/create-form.tsx
import { useEffect, useState } from "react";

// src/styles.ts
var BTN = {
  height: 26,
  padding: "0 12px",
  border: "0.5px solid var(--ava-border-l3)",
  borderRadius: "var(--ava-radius-sm, 10px)",
  background: "transparent",
  fontSize: 12,
  color: "var(--ava-label-primary)",
  cursor: "pointer"
};
var BTN_DANGER = { ...BTN, borderColor: "var(--ava-state-danger)", color: "var(--ava-state-danger)" };
var BTN_PRIMARY = {
  ...BTN,
  background: "var(--ava-state-business-primary)",
  color: "#fff",
  borderColor: "transparent"
};
var INPUT = {
  height: 28,
  padding: "0 8px",
  border: "0.5px solid var(--ava-border-l3)",
  borderRadius: "var(--ava-radius-sm, 10px)",
  background: "var(--ava-bg-layer-1)",
  color: "var(--ava-label-primary)",
  fontSize: 12,
  outline: "none",
  width: "100%",
  fontFamily: "var(--ava-font-ui)"
};
var TEXTAREA = { ...INPUT, height: 64, padding: "6px 8px", resize: "vertical", lineHeight: 1.5 };
var FIELD = { display: "flex", flexDirection: "column", gap: 4, flex: 1, minWidth: 160 };
var LABEL = { fontSize: 11, color: "var(--ava-label-tertiary)" };
var CHIP = (accent) => ({
  display: "inline-flex",
  alignItems: "center",
  padding: "1px 8px",
  borderRadius: "var(--ava-radius-pill)",
  fontSize: 11,
  whiteSpace: "nowrap",
  background: "var(--ava-nav-active)",
  color: accent || "var(--ava-label-secondary)"
});

// src/frequency-fields.tsx
import { jsx, jsxs } from "react/jsx-runtime";
function frequencyOf(freq) {
  if (freq.kind === "at") return { kind: "at", instant: localToRfc3339(freq.at) };
  if (freq.kind === "every") return { kind: "every", seconds: Math.max(60, (freq.minutes || 1) * 60) };
  if (freq.kind === "daily") return { kind: "daily", time: `${freq.dailyTime || "09:00"}:00`, time_zone: TZ };
  return { kind: "cron", expression: freq.cron.trim(), time_zone: TZ };
}
function freqComplete(freq) {
  if (freq.kind === "at") return /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}/.test(freq.at);
  if (freq.kind === "every") return (freq.minutes || 0) >= 1;
  if (freq.kind === "daily") return /^\d{2}:\d{2}$/.test(freq.dailyTime);
  return freq.cron.trim().length > 0;
}
function FieldF({ label, value, onChange, placeholder, inputType }) {
  return /* @__PURE__ */ jsxs("label", { style: FIELD, children: [
    /* @__PURE__ */ jsx("span", { style: LABEL, children: label }),
    /* @__PURE__ */ jsx(
      "input",
      {
        style: INPUT,
        value,
        placeholder,
        type: inputType || "text",
        onChange: (e) => onChange(e.target.value)
      }
    )
  ] });
}
function FrequencyFields({ freq, setFreq }) {
  const set = (patch) => setFreq({ ...freq, ...patch });
  const kind = freq.kind;
  return /* @__PURE__ */ jsxs("div", { style: { display: "flex", gap: "var(--ava-space-3, 10px)", flexWrap: "wrap", alignItems: "flex-end" }, children: [
    /* @__PURE__ */ jsx("div", { style: { display: "flex", gap: 6 }, children: ["at", "every", "daily", "cron"].map((k) => /* @__PURE__ */ jsx(
      "button",
      {
        style: {
          ...BTN,
          ...kind === k ? { borderColor: "var(--ava-state-business-primary)", color: "var(--ava-state-business-primary)" } : {}
        },
        onClick: () => set({ kind: k }),
        children: { at: "\u4E00\u6B21\u6027", every: "\u6BCF\u9694", daily: "\u6BCF\u5929", cron: "Cron" }[k]
      },
      k
    )) }),
    kind === "at" ? /* @__PURE__ */ jsx(FieldF, { label: "\u76EE\u6807\u65F6\u95F4", value: freq.at, onChange: (v) => set({ at: v }), inputType: "datetime-local" }) : null,
    kind === "every" ? /* @__PURE__ */ jsx(
      FieldF,
      {
        label: "\u95F4\u9694\uFF08\u5206\u949F\uFF0C\u22651\uFF09",
        value: String(freq.minutes ?? 30),
        onChange: (v) => set({ minutes: Math.max(1, Math.floor(Number(v) || 1)) }),
        inputType: "number"
      }
    ) : null,
    kind === "daily" ? /* @__PURE__ */ jsx(FieldF, { label: "\u65F6\u95F4\uFF08HH:mm\uFF09", value: freq.dailyTime, onChange: (v) => set({ dailyTime: v }), inputType: "time" }) : null,
    kind === "cron" ? /* @__PURE__ */ jsx(FieldF, { label: "Cron \u8868\u8FBE\u5F0F\uFF085/6 \u5B57\u6BB5\uFF09", value: freq.cron, onChange: (v) => set({ cron: v }), placeholder: "0 9 * * *" }) : null
  ] });
}

// src/create-form.tsx
import { jsx as jsx2, jsxs as jsxs2 } from "react/jsx-runtime";
function CreateForm({ agent, onSubmit, onCancel }) {
  const [title, setTitle] = useState("");
  const [prompt, setPrompt] = useState("");
  const [mode, setMode] = useState("independent");
  const [sessionId, setSessionId] = useState("");
  const [freq, setFreq] = useState({ kind: "at", at: "", minutes: 30, dailyTime: "", cron: "" });
  const [sessionOptions, setSessionOptions] = useState(null);
  useEffect(() => {
    if (mode !== "single" || sessionOptions) return;
    agent.listSessions().then((list) => {
      setSessionOptions(list);
      if (list.length > 0) setSessionId((cur) => cur || list[0].id);
    }).catch(() => setSessionOptions([]));
  }, [mode, sessionOptions, agent]);
  const freqValid = freqComplete(freq);
  const valid = title.trim() !== "" && prompt.trim() !== "" && freqValid && (mode === "independent" || sessionId !== "");
  return /* @__PURE__ */ jsxs2(
    "div",
    {
      style: {
        border: "0.5px solid var(--ava-border-l2)",
        borderRadius: "var(--ava-radius-lg, 16px)",
        padding: "var(--ava-space-4, 16px)",
        background: "var(--ava-bg-layer-1)",
        display: "flex",
        flexDirection: "column",
        gap: "var(--ava-space-3, 10px)"
      },
      children: [
        /* @__PURE__ */ jsx2("b", { style: { fontSize: 13 }, children: "\u65B0\u5EFA\u5B9A\u65F6\u4EFB\u52A1" }),
        /* @__PURE__ */ jsxs2("label", { style: FIELD, children: [
          /* @__PURE__ */ jsx2("span", { style: LABEL, children: "\u6807\u9898" }),
          /* @__PURE__ */ jsx2("input", { style: INPUT, value: title, onChange: (e) => setTitle(e.target.value), placeholder: "\u5982\uFF1A\u6BCF\u5929\u65E9\u4E0A\u6C47\u62A5\u8FDB\u5EA6" })
        ] }),
        /* @__PURE__ */ jsxs2("label", { style: FIELD, children: [
          /* @__PURE__ */ jsx2("span", { style: LABEL, children: "Prompt\uFF08\u5230\u671F\u6295\u9012\u7ED9\u4F1A\u8BDD\u6267\u884C\uFF09" }),
          /* @__PURE__ */ jsx2(
            "textarea",
            {
              style: TEXTAREA,
              value: prompt,
              onChange: (e) => setPrompt(e.target.value),
              placeholder: "\u8981 agent \u6267\u884C\u7684\u6307\u4EE4\u2026"
            }
          )
        ] }),
        /* @__PURE__ */ jsxs2("div", { style: { display: "flex", gap: 6, alignItems: "center", flexWrap: "wrap" }, children: [
          /* @__PURE__ */ jsx2("span", { style: LABEL, children: "\u6267\u884C\u6A21\u5F0F" }),
          /* @__PURE__ */ jsx2(
            "button",
            {
              style: {
                ...BTN,
                ...mode === "independent" ? { borderColor: "var(--ava-state-business-primary)", color: "var(--ava-state-business-primary)" } : {}
              },
              onClick: () => setMode("independent"),
              children: "\u72EC\u7ACB\u4F1A\u8BDD\uFF08\u5230\u671F\u65B0\u5EFA\uFF09"
            }
          ),
          /* @__PURE__ */ jsx2(
            "button",
            {
              style: {
                ...BTN,
                ...mode === "single" ? { borderColor: "var(--ava-state-business-primary)", color: "var(--ava-state-business-primary)" } : {}
              },
              onClick: () => setMode("single"),
              children: "\u7ED1\u5B9A\u4F1A\u8BDD"
            }
          ),
          mode === "single" ? sessionOptions === null ? /* @__PURE__ */ jsx2("span", { style: LABEL, children: "\u4F1A\u8BDD\u52A0\u8F7D\u4E2D\u2026" }) : /* @__PURE__ */ jsx2("select", { style: { ...INPUT, width: 260 }, value: sessionId, onChange: (e) => setSessionId(e.target.value), children: sessionOptions.map((s) => /* @__PURE__ */ jsx2("option", { value: s.id, children: `${s.title || s.id}`.slice(0, 40) }, s.id)) }) : null
        ] }),
        /* @__PURE__ */ jsx2(FrequencyFields, { freq, setFreq }),
        /* @__PURE__ */ jsxs2("div", { style: { display: "flex", justifyContent: "flex-end", gap: "var(--ava-space-2, 8px)" }, children: [
          /* @__PURE__ */ jsx2("button", { style: BTN, onClick: onCancel, children: "\u53D6\u6D88" }),
          /* @__PURE__ */ jsx2(
            "button",
            {
              style: BTN_PRIMARY,
              disabled: !valid,
              onClick: () => onSubmit({
                title: title.trim(),
                prompt: prompt.trim(),
                ...mode === "independent" ? { sessionMode: "independent" } : { sessionId },
                frequency: frequencyOf(freq)
              }),
              children: "\u521B\u5EFA"
            }
          )
        ] })
      ]
    }
  );
}

// src/edit-form.tsx
import { useState as useState2 } from "react";
import { jsx as jsx3, jsxs as jsxs3 } from "react/jsx-runtime";
function EditForm({ task, onSubmit, onCancel }) {
  const [title, setTitle] = useState2(task.title);
  const [prompt, setPrompt] = useState2(task.prompt);
  const [editWhen, setEditWhen] = useState2(false);
  const [freq, setFreq] = useState2({ kind: "every", at: "", minutes: 30, dailyTime: "", cron: "" });
  const valid = title.trim() !== "" && prompt.trim() !== "" && (!editWhen || freqComplete(freq));
  return /* @__PURE__ */ jsxs3(
    "div",
    {
      style: {
        border: "0.5px solid var(--ava-border-l3)",
        borderRadius: "var(--ava-radius-lg, 16px)",
        padding: "var(--ava-space-4, 16px)",
        background: "var(--ava-bg-layer-1)",
        display: "flex",
        flexDirection: "column",
        gap: "var(--ava-space-3, 10px)"
      },
      children: [
        /* @__PURE__ */ jsx3("b", { style: { fontSize: 13 }, children: "\u7F16\u8F91\u5B9A\u65F6\u4EFB\u52A1" }),
        /* @__PURE__ */ jsxs3("label", { style: FIELD, children: [
          /* @__PURE__ */ jsx3("span", { style: LABEL, children: "\u6807\u9898" }),
          /* @__PURE__ */ jsx3("input", { style: INPUT, value: title, onChange: (e) => setTitle(e.target.value) })
        ] }),
        /* @__PURE__ */ jsxs3("label", { style: FIELD, children: [
          /* @__PURE__ */ jsx3("span", { style: LABEL, children: "\u6307\u4EE4\uFF08\u5230\u671F\u6295\u9012\u7ED9\u4F1A\u8BDD\u6267\u884C\uFF09" }),
          /* @__PURE__ */ jsx3("textarea", { style: TEXTAREA, value: prompt, onChange: (e) => setPrompt(e.target.value) })
        ] }),
        /* @__PURE__ */ jsxs3("div", { style: { display: "flex", gap: 6, alignItems: "center", flexWrap: "wrap" }, children: [
          /* @__PURE__ */ jsx3("span", { style: LABEL, children: "\u8BA1\u5212\u65F6\u95F4" }),
          /* @__PURE__ */ jsxs3(
            "button",
            {
              style: {
                ...BTN,
                ...editWhen ? {} : { borderColor: "var(--ava-state-business-primary)", color: "var(--ava-state-business-primary)" }
              },
              onClick: () => setEditWhen(false),
              children: [
                "\u4FDD\u6301\u4E0D\u53D8\uFF08\u4E0B\u6B21 ",
                fmtWhen(task.scheduledAt),
                "\uFF09"
              ]
            }
          ),
          /* @__PURE__ */ jsx3(
            "button",
            {
              style: {
                ...BTN,
                ...editWhen ? { borderColor: "var(--ava-state-business-primary)", color: "var(--ava-state-business-primary)" } : {}
              },
              onClick: () => setEditWhen(true),
              children: "\u4FEE\u6539\u8BA1\u5212"
            }
          )
        ] }),
        editWhen ? /* @__PURE__ */ jsx3(FrequencyFields, { freq, setFreq }) : null,
        /* @__PURE__ */ jsxs3("div", { style: { fontSize: 11, color: "var(--ava-label-tertiary)" }, children: [
          "\u6267\u884C\u6A21\u5F0F\u4E0E\u7ED1\u5B9A\u4F1A\u8BDD\u521B\u5EFA\u540E\u4E0D\u53EF\u4FEE\u6539\uFF1A",
          task.sessionMode === "independent" ? "\u72EC\u7ACB\u4F1A\u8BDD\uFF08\u5230\u671F\u65B0\u5EFA\uFF09" : "\u7ED1\u5B9A\u4F1A\u8BDD " + task.sessionId.slice(0, 12) + "\u2026"
        ] }),
        /* @__PURE__ */ jsxs3("div", { style: { display: "flex", justifyContent: "flex-end", gap: "var(--ava-space-2, 8px)" }, children: [
          /* @__PURE__ */ jsx3("button", { style: BTN, onClick: onCancel, children: "\u53D6\u6D88" }),
          /* @__PURE__ */ jsx3(
            "button",
            {
              style: BTN_PRIMARY,
              disabled: !valid,
              onClick: () => onSubmit({
                title: title.trim(),
                prompt: prompt.trim(),
                ...editWhen ? { frequency: frequencyOf(freq) } : {}
              }),
              children: "\u4FDD\u5B58"
            }
          )
        ] })
      ]
    }
  );
}

// src/popconfirm.tsx
import { useEffect as useEffect2, useLayoutEffect, useRef, useState as useState3 } from "react";
import { jsx as jsx4, jsxs as jsxs4 } from "react/jsx-runtime";
function Popconfirm({ title, onCancel, onConfirm }) {
  const ref = useRef(null);
  const [pos, setPos] = useState3(null);
  useEffect2(() => {
    const onDoc = (e) => {
      if (ref.current && !ref.current.contains(e.target)) onCancel();
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [onCancel]);
  useLayoutEffect(() => {
    const el = ref.current;
    const anchor = el?.parentElement;
    if (!el || !anchor) return;
    const r = anchor.getBoundingClientRect();
    const w = el.offsetWidth;
    const bh = el.offsetHeight;
    const left = Math.max(8, Math.min(r.right - w, window.innerWidth - w - 8));
    let top = r.bottom + 6;
    if (top + bh > window.innerHeight - 8) top = Math.max(8, r.top - bh - 6);
    setPos({ top, left });
  }, []);
  return /* @__PURE__ */ jsxs4(
    "div",
    {
      ref,
      role: "dialog",
      "aria-label": "\u786E\u8BA4\u64CD\u4F5C",
      onClick: (e) => e.stopPropagation(),
      style: {
        position: "fixed",
        zIndex: 40,
        minWidth: 224,
        maxWidth: 268,
        padding: 10,
        border: "0.5px solid var(--ava-border-l3)",
        borderRadius: "var(--ava-radius-sm, 10px)",
        background: "var(--ava-bg-layer-2)",
        boxShadow: "var(--ava-shadow-panel)",
        fontSize: 12,
        color: "var(--ava-label-primary)",
        ...pos ? { top: pos.top, left: pos.left } : { visibility: "hidden" }
      },
      children: [
        /* @__PURE__ */ jsx4("div", { style: { lineHeight: 1.6, wordBreak: "break-word" }, children: title }),
        /* @__PURE__ */ jsxs4("div", { style: { display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 8 }, children: [
          /* @__PURE__ */ jsx4("button", { style: { ...BTN, color: "var(--ava-label-secondary)" }, onClick: onCancel, children: "\u53D6\u6D88" }),
          /* @__PURE__ */ jsx4(
            "button",
            {
              style: BTN_DANGER,
              onClick: () => {
                onCancel();
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

// src/tasks-view.tsx
import { jsx as jsx5, jsxs as jsxs5 } from "react/jsx-runtime";
function TasksView({ agent }) {
  const [confirmDelete, setConfirmDelete] = useState4(() => readConfig().confirmDelete);
  const [tasks, setTasks] = useState4(null);
  const [err, setErr] = useState4("");
  const [note, setNote] = useState4("");
  const [confirmId, setConfirmId] = useState4("");
  const [editingId, setEditingId] = useState4("");
  const [creating, setCreating] = useState4(false);
  const [busyId, setBusyId] = useState4("");
  const load = useCallback(() => {
    agent.listTasks().then((r) => setTasks(Array.isArray(r.tasks) ? r.tasks : [])).catch((e) => setErr(e.message || String(e)));
  }, [agent]);
  useEffect3(() => {
    load();
    const timer = setInterval(() => {
      setConfirmDelete(readConfig().confirmDelete);
      load();
    }, 5e3);
    return () => clearInterval(timer);
  }, [load]);
  const run = async (fn, okNote) => {
    setErr("");
    setNote("");
    try {
      await fn();
      if (okNote) setNote(okNote);
      load();
      return true;
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
      return false;
    }
  };
  const toggle = (task) => {
    void run(
      () => agent.toggleTask(task.id, !task.enabled),
      task.enabled ? `\u5DF2\u505C\u7528\u300C${task.title}\u300D` : `\u5DF2\u542F\u7528\u300C${task.title}\u300D`
    );
  };
  const runNow = async (task) => {
    setBusyId(task.id);
    await run(() => agent.runTask(task.id), `\u300C${task.title}\u300D\u5DF2\u6295\u9012`);
    setBusyId("");
  };
  const removeTask = (task) => {
    void run(() => agent.deleteTask(task.id), `\u5DF2\u5220\u9664\u300C${task.title}\u300D`);
  };
  if (err && !tasks) {
    return /* @__PURE__ */ jsxs5("div", { style: { padding: "var(--ava-space-5, 24px)", color: "var(--ava-state-danger)", fontSize: 13 }, children: [
      "\u52A0\u8F7D\u5931\u8D25\uFF1A",
      err,
      /* @__PURE__ */ jsx5("button", { style: { ...BTN, marginLeft: 12 }, onClick: load, children: "\u91CD\u8BD5" })
    ] });
  }
  if (!tasks) {
    return /* @__PURE__ */ jsx5("div", { style: { padding: "var(--ava-space-5, 24px)", color: "var(--ava-label-tertiary)", fontSize: 13 }, children: "\u52A0\u8F7D\u4E2D\u2026" });
  }
  const deleteGuard = (task) => confirmDelete ? () => setConfirmId(task.id) : () => removeTask(task);
  return /* @__PURE__ */ jsxs5(
    "div",
    {
      style: {
        padding: "var(--ava-space-5, 24px)",
        overflow: "auto",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        gap: "var(--ava-space-4, 16px)"
      },
      children: [
        /* @__PURE__ */ jsxs5("div", { style: { display: "flex", alignItems: "center", gap: "var(--ava-space-3, 10px)" }, children: [
          /* @__PURE__ */ jsx5("h2", { style: { margin: 0, fontSize: 16 }, children: "\u5B9A\u65F6\u4EFB\u52A1" }),
          /* @__PURE__ */ jsxs5("span", { style: { fontSize: 12, color: "var(--ava-label-tertiary)" }, children: [
            tasks.length,
            " \u4E2A\u4EFB\u52A1 \xB7 5s \u81EA\u52A8\u5237\u65B0"
          ] }),
          /* @__PURE__ */ jsx5("span", { style: { flex: 1 } }),
          /* @__PURE__ */ jsx5("button", { style: BTN_PRIMARY, onClick: () => setCreating(!creating), children: creating ? "\u6536\u8D77" : "+ \u65B0\u5EFA\u4EFB\u52A1" })
        ] }),
        note || err ? /* @__PURE__ */ jsx5("div", { style: { fontSize: 12, color: err ? "var(--ava-state-danger)" : "var(--ava-state-success)" }, children: err || note }) : null,
        creating ? /* @__PURE__ */ jsx5(
          CreateForm,
          {
            agent,
            onSubmit: async (fields) => {
              const ok = await run(() => agent.createTask(fields), `\u5DF2\u521B\u5EFA\u300C${fields.title}\u300D`);
              if (ok) setCreating(false);
            },
            onCancel: () => setCreating(false)
          }
        ) : null,
        tasks.map((task) => /* @__PURE__ */ jsxs5(
          "div",
          {
            style: {
              border: "0.5px solid var(--ava-border-l2)",
              borderRadius: "var(--ava-radius-lg, 16px)",
              padding: "var(--ava-space-4, 16px)",
              background: "var(--ava-bg-layer-1)",
              display: "flex",
              flexDirection: "column",
              gap: 6,
              opacity: task.enabled ? 1 : 0.55
            },
            children: [
              /* @__PURE__ */ jsxs5("div", { style: { display: "flex", alignItems: "center", gap: "var(--ava-space-2, 8px)", flexWrap: "wrap" }, children: [
                /* @__PURE__ */ jsx5("b", { style: { fontSize: 13 }, children: task.title }),
                /* @__PURE__ */ jsx5("span", { style: CHIP(task.enabled ? "var(--ava-state-success)" : "var(--ava-label-tertiary)"), children: task.enabled ? "\u542F\u7528\u4E2D" : "\u5DF2\u505C\u7528" }),
                /* @__PURE__ */ jsx5("span", { style: CHIP(), children: KIND_LABEL[task.kind] || task.kind }),
                /* @__PURE__ */ jsx5("span", { style: CHIP(), children: task.sessionMode === "independent" ? "\u72EC\u7ACB\u4F1A\u8BDD" : "\u7ED1\u5B9A\u4F1A\u8BDD" }),
                /* @__PURE__ */ jsx5("span", { style: { flex: 1 } }),
                /* @__PURE__ */ jsx5(
                  "button",
                  {
                    style: BTN,
                    onClick: () => setEditingId(editingId === task.id ? "" : task.id),
                    children: editingId === task.id ? "\u6536\u8D77" : "\u7F16\u8F91"
                  }
                ),
                /* @__PURE__ */ jsx5("button", { style: BTN, disabled: busyId === task.id, onClick: () => void runNow(task), children: busyId === task.id ? "\u6267\u884C\u4E2D\u2026" : "\u7ACB\u5373\u6267\u884C" }),
                /* @__PURE__ */ jsx5("button", { style: BTN, onClick: () => toggle(task), children: task.enabled ? "\u505C\u7528" : "\u542F\u7528" }),
                /* @__PURE__ */ jsxs5("span", { style: { position: "relative" }, children: [
                  /* @__PURE__ */ jsx5("button", { style: BTN_DANGER, onClick: deleteGuard(task), children: "\u5220\u9664" }),
                  confirmId === task.id ? /* @__PURE__ */ jsx5(
                    Popconfirm,
                    {
                      title: `\u5220\u9664\u4EFB\u52A1\u300C${task.title}\u300D\uFF1F`,
                      onCancel: () => setConfirmId(""),
                      onConfirm: () => removeTask(task)
                    }
                  ) : null
                ] })
              ] }),
              /* @__PURE__ */ jsx5(
                "div",
                {
                  style: {
                    fontSize: 12,
                    color: "var(--ava-label-secondary)",
                    lineHeight: 1.6,
                    whiteSpace: "pre-wrap",
                    wordBreak: "break-word",
                    display: "-webkit-box",
                    WebkitLineClamp: 2,
                    WebkitBoxOrient: "vertical",
                    overflow: "hidden"
                  },
                  children: task.prompt
                }
              ),
              /* @__PURE__ */ jsxs5(
                "div",
                {
                  style: {
                    display: "flex",
                    gap: "var(--ava-space-4, 16px)",
                    fontSize: 11,
                    color: "var(--ava-label-tertiary)",
                    flexWrap: "wrap"
                  },
                  children: [
                    /* @__PURE__ */ jsxs5("span", { children: [
                      "\u4E0B\u6B21\u89E6\u53D1\uFF1A",
                      /* @__PURE__ */ jsx5(
                        "b",
                        {
                          style: {
                            fontFamily: "var(--ava-font-mono, monospace)",
                            fontWeight: 500,
                            color: task.enabled ? "var(--ava-label-secondary)" : "var(--ava-label-tertiary)"
                          },
                          children: fmtWhen(task.scheduledAt)
                        }
                      )
                    ] }),
                    task.sessionMode !== "independent" && task.sessionId ? /* @__PURE__ */ jsxs5("span", { style: { fontFamily: "var(--ava-font-mono, monospace)" }, children: [
                      "\u4F1A\u8BDD ",
                      task.sessionId.slice(0, 12),
                      "\u2026"
                    ] }) : null,
                    task.lastDeliveredAt ? /* @__PURE__ */ jsxs5("span", { children: [
                      "\u4E0A\u6B21\u9012\u9001\uFF1A",
                      fmtWhen(task.lastDeliveredAt)
                    ] }) : null
                  ]
                }
              ),
              editingId === task.id ? /* @__PURE__ */ jsx5(
                EditForm,
                {
                  task,
                  onCancel: () => setEditingId(""),
                  onSubmit: async (input) => {
                    const ok = await run(() => agent.updateTask(task.id, input), `\u5DF2\u4FDD\u5B58\u300C${input.title ?? task.title}\u300D`);
                    if (ok) setEditingId("");
                  }
                }
              ) : null
            ]
          },
          task.id
        )),
        tasks.length === 0 && !creating ? /* @__PURE__ */ jsx5("div", { style: { padding: "var(--ava-space-5, 24px)", textAlign: "center", color: "var(--ava-label-tertiary)", fontSize: 13 }, children: "\u5C1A\u65E0\u5B9A\u65F6\u4EFB\u52A1\u2014\u2014\u53F3\u4E0A\u89D2\u300C+ \u65B0\u5EFA\u4EFB\u52A1\u300D\u5F00\u59CB\u521B\u5EFA\u3002" }) : null
      ]
    }
  );
}

// src/main.tsx
import { jsx as jsx7 } from "react/jsx-runtime";
function setup(ctx) {
  ctx.registerSettingsPanel(() => Promise.resolve().then(() => (init_settings_panel(), settings_panel_exports)));
  const disposers = [
    // lucide clock：定时语义
    ctx.slots.inject("nav.sidebar", {
      order: 8,
      viewId: "task-manager",
      title: "\u5B9A\u65F6\u4EFB\u52A1",
      icon: "M12 6v6l4.5 3 M21.5 12a9.5 9.5 0 1 1-19 0 9.5 9.5 0 0 1 19 0z"
    }),
    ctx.slots.inject("surface.main", {
      viewId: "task-manager",
      title: "\u5B9A\u65F6\u4EFB\u52A1",
      component: async () => ({ default: () => /* @__PURE__ */ jsx7(TasksView, { agent: ctx.agent }) })
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
