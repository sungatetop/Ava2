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
    CFG_KEY = "model-manager:cfg";
    DEFAULTS = { confirmDelete: true };
  }
});

// src/settings-panel.tsx
var settings_panel_exports = {};
__export(settings_panel_exports, {
  default: () => SettingsPanel
});
import { useState as useState3 } from "react";
import { jsx as jsx7, jsxs as jsxs7 } from "react/jsx-runtime";
function SettingsPanel() {
  const [confirmDelete, setConfirmDelete] = useState3(() => readConfig().confirmDelete);
  return /* @__PURE__ */ jsxs7("div", { style: { display: "flex", flexDirection: "column", gap: 10, padding: "4px 0", fontSize: 13 }, children: [
    /* @__PURE__ */ jsxs7("label", { style: { display: "flex", alignItems: "center", gap: 8, cursor: "pointer" }, children: [
      /* @__PURE__ */ jsx7(
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
    /* @__PURE__ */ jsx7("div", { style: { fontSize: 12, color: "var(--ava-label-tertiary)" }, children: "\u914D\u7F6E\u7531\u672C App \u81EA\u884C\u5B58\u50A8\uFF08localStorage\uFF09\uFF0C\u5373\u65F6\u751F\u6548\u3002" })
  ] });
}
var init_settings_panel = __esm({
  "src/settings-panel.tsx"() {
    "use strict";
    init_config();
  }
});

// src/models-view.tsx
init_config();
import { useCallback, useEffect as useEffect2, useState as useState2 } from "react";

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
var CARD = {
  border: "0.5px solid var(--ava-border-l2)",
  borderRadius: "var(--ava-radius-lg, 16px)",
  padding: "var(--ava-space-4, 16px)",
  background: "var(--ava-bg-layer-1)"
};

// src/field.tsx
import { jsx, jsxs } from "react/jsx-runtime";
function Field({ label, value, onChange, placeholder, password }) {
  return /* @__PURE__ */ jsxs("label", { style: FIELD, children: [
    /* @__PURE__ */ jsx("span", { style: LABEL, children: label }),
    /* @__PURE__ */ jsx(
      "input",
      {
        style: INPUT,
        value,
        placeholder,
        type: password ? "password" : "text",
        onChange: (e) => onChange(e.target.value)
      }
    )
  ] });
}

// src/test-badge.tsx
import { jsx as jsx2, jsxs as jsxs2 } from "react/jsx-runtime";
function TestBadge({ state }) {
  if (!state) return null;
  if (state.loading) return /* @__PURE__ */ jsx2("span", { style: { fontSize: 11, color: "var(--ava-label-tertiary)" }, children: "\u6D4B\u8BD5\u4E2D\u2026" });
  const r = state.result;
  if (!r) return null;
  if (r.ok) {
    return /* @__PURE__ */ jsxs2("span", { style: { fontSize: 11, color: "var(--ava-state-success)" }, children: [
      "\u8FDE\u901A ",
      r.latencyMs,
      "ms"
    ] });
  }
  return /* @__PURE__ */ jsxs2(
    "span",
    {
      title: r.error,
      style: {
        fontSize: 11,
        color: "var(--ava-state-danger)",
        maxWidth: 260,
        overflow: "hidden",
        textOverflow: "ellipsis",
        whiteSpace: "nowrap",
        display: "inline-block",
        verticalAlign: "bottom"
      },
      children: [
        "\u5931\u8D25\uFF1A",
        r.error || "\u672A\u77E5\u9519\u8BEF"
      ]
    }
  );
}

// src/add-model-form.tsx
import { jsx as jsx3, jsxs as jsxs3 } from "react/jsx-runtime";
function AddModelForm({ providers, fields, onChange, onSubmit, onTest, testState }) {
  const set = (patch) => onChange({ ...fields, ...patch });
  const isNew = fields.newProvider;
  const canTest = Boolean(isNew ? fields.baseUrl && fields.apiKey && fields.id : fields.id);
  const valid = Boolean(isNew ? fields.provider && fields.baseUrl && fields.apiKey : fields.provider) && Boolean(fields.id);
  return /* @__PURE__ */ jsxs3("div", { style: { ...CARD, display: "flex", flexDirection: "column", gap: "var(--ava-space-3, 10px)" }, children: [
    /* @__PURE__ */ jsx3("b", { style: { fontSize: 13 }, children: "\u6DFB\u52A0\u6A21\u578B" }),
    /* @__PURE__ */ jsxs3("div", { style: { display: "flex", gap: "var(--ava-space-2, 8px)", flexWrap: "wrap" }, children: [
      providers.map((id) => /* @__PURE__ */ jsx3(
        "button",
        {
          style: {
            ...BTN,
            ...fields.provider === id && !isNew ? { borderColor: "var(--ava-state-business-primary)", color: "var(--ava-state-business-primary)" } : {}
          },
          onClick: () => set({ provider: id, newProvider: false }),
          children: id
        },
        id
      )),
      /* @__PURE__ */ jsx3(
        "button",
        {
          style: {
            ...BTN,
            ...isNew ? { borderColor: "var(--ava-state-business-primary)", color: "var(--ava-state-business-primary)" } : {}
          },
          onClick: () => set({ provider: "", newProvider: true }),
          children: "\u65B0\u5EFA\u4F9B\u5E94\u5546\u2026"
        }
      )
    ] }),
    /* @__PURE__ */ jsxs3("div", { style: { display: "flex", gap: "var(--ava-space-3, 10px)", flexWrap: "wrap" }, children: [
      isNew ? /* @__PURE__ */ jsx3(Field, { label: "\u4F9B\u5E94\u5546 ID", value: fields.provider, onChange: (v) => set({ provider: v }), placeholder: "\u5982 my-provider" }) : null,
      isNew ? /* @__PURE__ */ jsx3(
        Field,
        {
          label: "Base URL",
          value: fields.baseUrl,
          onChange: (v) => set({ baseUrl: v }),
          placeholder: "https://api.example.com/v1"
        }
      ) : null,
      isNew ? /* @__PURE__ */ jsx3(Field, { label: "API Key", value: fields.apiKey, onChange: (v) => set({ apiKey: v }), placeholder: "sk-\u2026", password: true }) : null,
      /* @__PURE__ */ jsx3(Field, { label: "\u6A21\u578B ID", value: fields.id, onChange: (v) => set({ id: v }), placeholder: "provider \u4FA7\u6A21\u578B\u540D\uFF0C\u5982 gpt-4o-mini" }),
      /* @__PURE__ */ jsx3(Field, { label: "\u663E\u793A\u540D\uFF08\u53EF\u9009\uFF09", value: fields.name, onChange: (v) => set({ name: v }) }),
      /* @__PURE__ */ jsx3(Field, { label: "\u63CF\u8FF0\uFF08\u53EF\u9009\uFF09", value: fields.description, onChange: (v) => set({ description: v }) })
    ] }),
    /* @__PURE__ */ jsxs3("div", { style: { display: "flex", alignItems: "center", gap: "var(--ava-space-2, 8px)" }, children: [
      /* @__PURE__ */ jsx3("button", { style: BTN, disabled: !canTest, onClick: () => onTest(fields), children: "\u6D4B\u8BD5\u8FDE\u63A5" }),
      /* @__PURE__ */ jsx3(TestBadge, { state: testState }),
      /* @__PURE__ */ jsx3("span", { style: { flex: 1 } }),
      /* @__PURE__ */ jsx3("button", { style: BTN, onClick: () => onChange(null), children: "\u53D6\u6D88" }),
      /* @__PURE__ */ jsx3(
        "button",
        {
          style: BTN_PRIMARY,
          disabled: !valid,
          onClick: () => onSubmit({
            provider: fields.provider,
            id: fields.id,
            ...fields.name ? { name: fields.name } : {},
            ...fields.description ? { description: fields.description } : {},
            ...isNew ? { baseUrl: fields.baseUrl, apiKey: fields.apiKey } : {}
          }),
          children: "\u4FDD\u5B58"
        }
      )
    ] })
  ] });
}

// src/add-provider-form.tsx
import { jsx as jsx4, jsxs as jsxs4 } from "react/jsx-runtime";
function AddProviderForm({ fields, onChange, onSubmit }) {
  const set = (patch) => onChange({ ...fields, ...patch });
  const valid = Boolean(fields.id && fields.baseUrl && fields.apiKey);
  return /* @__PURE__ */ jsxs4("div", { style: { ...CARD, display: "flex", flexDirection: "column", gap: "var(--ava-space-3, 10px)" }, children: [
    /* @__PURE__ */ jsx4("b", { style: { fontSize: 13 }, children: "\u6DFB\u52A0\u4F9B\u5E94\u5546" }),
    /* @__PURE__ */ jsxs4("div", { style: { display: "flex", gap: "var(--ava-space-3, 10px)", flexWrap: "wrap" }, children: [
      /* @__PURE__ */ jsx4(Field, { label: "ID", value: fields.id, onChange: (v) => set({ id: v }), placeholder: "\u5982 openai" }),
      /* @__PURE__ */ jsx4(Field, { label: "\u663E\u793A\u540D\uFF08\u53EF\u9009\uFF09", value: fields.name, onChange: (v) => set({ name: v }) }),
      /* @__PURE__ */ jsx4(
        Field,
        {
          label: "Base URL",
          value: fields.baseUrl,
          onChange: (v) => set({ baseUrl: v }),
          placeholder: "https://api.example.com/v1"
        }
      ),
      /* @__PURE__ */ jsx4(Field, { label: "API Key", value: fields.apiKey, onChange: (v) => set({ apiKey: v }), placeholder: "sk-\u2026", password: true })
    ] }),
    /* @__PURE__ */ jsxs4("div", { style: { display: "flex", justifyContent: "flex-end", gap: "var(--ava-space-2, 8px)" }, children: [
      /* @__PURE__ */ jsx4("button", { style: BTN, onClick: () => onChange(null), children: "\u53D6\u6D88" }),
      /* @__PURE__ */ jsx4(
        "button",
        {
          style: BTN_PRIMARY,
          disabled: !valid,
          onClick: () => onSubmit({
            id: fields.id,
            ...fields.name ? { name: fields.name } : {},
            baseUrl: fields.baseUrl,
            apiKey: fields.apiKey
          }),
          children: "\u4FDD\u5B58"
        }
      )
    ] })
  ] });
}

// src/popconfirm.tsx
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { jsx as jsx5, jsxs as jsxs5 } from "react/jsx-runtime";
function Popconfirm({ title, onCancel, onConfirm }) {
  const ref = useRef(null);
  const [pos, setPos] = useState(null);
  useEffect(() => {
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
  return /* @__PURE__ */ jsxs5(
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
        /* @__PURE__ */ jsx5("div", { style: { lineHeight: 1.6, wordBreak: "break-word" }, children: title }),
        /* @__PURE__ */ jsxs5("div", { style: { display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 8 }, children: [
          /* @__PURE__ */ jsx5("button", { style: { ...BTN, color: "var(--ava-label-secondary)" }, onClick: onCancel, children: "\u53D6\u6D88" }),
          /* @__PURE__ */ jsx5(
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

// src/models-view.tsx
import { jsx as jsx6, jsxs as jsxs6 } from "react/jsx-runtime";
function ModelsView({ agent }) {
  const [data, setData] = useState2(null);
  const [err, setErr] = useState2("");
  const [note, setNote] = useState2("");
  const [confirmKey, setConfirmKey] = useState2("");
  const [form, setForm] = useState2(null);
  const [tests, setTests] = useState2({});
  const load = useCallback(() => {
    agent.listModels().then((r) => setData({ defaultModel: r.defaultModel || "", providers: Array.isArray(r.providers) ? r.providers : [] })).catch((e) => setErr(e instanceof Error ? e.message : String(e)));
  }, [agent]);
  useEffect2(() => {
    load();
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
  const setDefault = (model) => {
    void run(() => agent.setDefaultModel(model), `\u9ED8\u8BA4\u6A21\u578B\u5DF2\u5207\u6362\u4E3A ${model}`);
  };
  const removeModel = (provider, model) => {
    void run(() => agent.removeModel(provider, model), `\u5DF2\u5220\u9664\u6A21\u578B ${model}`);
  };
  const removeProvider = (id) => {
    void run(() => agent.removeProvider(id), `\u5DF2\u5220\u9664\u4F9B\u5E94\u5546 ${id}`);
  };
  const testRow = async (provider, model) => {
    const key = `${provider}/${model}`;
    setTests((t) => ({ ...t, [key]: { loading: true } }));
    try {
      const result = await agent.testModel({ provider, model });
      setTests((t) => ({ ...t, [key]: { result } }));
    } catch (e) {
      const result = { ok: false, latencyMs: 0, error: e instanceof Error ? e.message : String(e) };
      setTests((t) => ({ ...t, [key]: { result } }));
    }
  };
  const testForm = async (fields) => {
    setTests((t) => ({ ...t, __form: { loading: true } }));
    try {
      const result = await agent.testModel({ baseUrl: fields.baseUrl, apiKey: fields.apiKey, model: fields.id });
      setTests((t) => ({ ...t, __form: { result } }));
    } catch (e) {
      const result = { ok: false, latencyMs: 0, error: e instanceof Error ? e.message : String(e) };
      setTests((t) => ({ ...t, __form: { result } }));
    }
  };
  const submitModel = async (input) => {
    const ok = await run(() => agent.addModel(input), `\u5DF2\u6DFB\u52A0\u6A21\u578B ${input.id}`);
    if (ok) setForm(null);
  };
  const submitProvider = async (input) => {
    const ok = await run(() => agent.addProvider(input), `\u5DF2\u6DFB\u52A0\u4F9B\u5E94\u5546 ${input.id}`);
    if (ok) setForm(null);
  };
  const deleteGuard = (key, action) => {
    if (readConfig().confirmDelete) setConfirmKey(key);
    else action();
  };
  if (err && !data) {
    return /* @__PURE__ */ jsxs6("div", { style: { padding: "var(--ava-space-5, 24px)", color: "var(--ava-state-danger)", fontSize: 13 }, children: [
      "\u52A0\u8F7D\u5931\u8D25\uFF1A",
      err,
      /* @__PURE__ */ jsx6("button", { style: { ...BTN, marginLeft: 12 }, onClick: load, children: "\u91CD\u8BD5" })
    ] });
  }
  if (!data) {
    return /* @__PURE__ */ jsx6("div", { style: { padding: "var(--ava-space-5, 24px)", color: "var(--ava-label-tertiary)", fontSize: 13 }, children: "\u52A0\u8F7D\u4E2D\u2026" });
  }
  return /* @__PURE__ */ jsxs6(
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
        /* @__PURE__ */ jsxs6("div", { style: { display: "flex", alignItems: "center", gap: "var(--ava-space-3, 10px)" }, children: [
          /* @__PURE__ */ jsx6("h2", { style: { margin: 0, fontSize: 16 }, children: "\u6A21\u578B\u7BA1\u7406" }),
          /* @__PURE__ */ jsxs6(
            "span",
            {
              title: "\u65B0\u4F1A\u8BDD\u7684\u521D\u59CB\u6A21\u578B\uFF08PUT /v1/models/default \u6301\u4E45\u5316\uFF1B\u5DF2\u6709\u4F1A\u8BDD\u5728\u4F1A\u8BDD\u5185\u5207\u6362\uFF09",
              style: CHIP(),
              children: [
                "\u9ED8\u8BA4\u6A21\u578B\uFF1A",
                /* @__PURE__ */ jsx6("b", { style: { fontFamily: "var(--ava-font-mono, monospace)", fontWeight: 500, marginLeft: 4 }, children: data.defaultModel || "\uFF08\u672A\u914D\u7F6E\uFF09" })
              ]
            }
          ),
          /* @__PURE__ */ jsx6("span", { style: { flex: 1 } }),
          /* @__PURE__ */ jsx6(
            "button",
            {
              style: BTN,
              onClick: () => setForm(form?.kind === "provider" ? null : { kind: "provider", id: "", name: "", baseUrl: "", apiKey: "" }),
              children: "+ \u6DFB\u52A0\u4F9B\u5E94\u5546"
            }
          ),
          /* @__PURE__ */ jsx6(
            "button",
            {
              style: BTN_PRIMARY,
              onClick: () => setForm(
                form?.kind === "model" ? null : { kind: "model", provider: "", newProvider: false, id: "", name: "", description: "", baseUrl: "", apiKey: "" }
              ),
              children: "+ \u6DFB\u52A0\u6A21\u578B"
            }
          )
        ] }),
        err || note ? /* @__PURE__ */ jsx6("div", { style: { fontSize: 12, color: err ? "var(--ava-state-danger)" : "var(--ava-state-success)" }, children: err || note }) : null,
        data.providers.map((provider) => /* @__PURE__ */ jsx6(
          ProviderCard,
          {
            provider,
            defaultModel: data.defaultModel,
            confirmKey,
            tests,
            onSetDefault: setDefault,
            onRemoveModel: removeModel,
            onRemoveProvider: removeProvider,
            onTestRow: testRow,
            onConfirmCancel: () => setConfirmKey(""),
            deleteGuard
          },
          provider.id
        )),
        data.providers.length === 0 ? /* @__PURE__ */ jsx6("div", { style: { padding: "var(--ava-space-5, 24px)", textAlign: "center", color: "var(--ava-label-tertiary)", fontSize: 13 }, children: "\u5C1A\u65E0\u4F9B\u5E94\u5546\u2014\u2014\u53F3\u4E0A\u89D2\u300C+ \u6DFB\u52A0\u4F9B\u5E94\u5546\u300D\u6216\u300C+ \u6DFB\u52A0\u6A21\u578B\u300D\u5F00\u59CB\u914D\u7F6E\u3002" }) : null,
        form?.kind === "model" ? /* @__PURE__ */ jsx6(
          AddModelForm,
          {
            providers: data.providers.map((p) => p.id),
            fields: form,
            onChange: setForm,
            onSubmit: submitModel,
            onTest: testForm,
            testState: tests.__form
          }
        ) : null,
        form?.kind === "provider" ? /* @__PURE__ */ jsx6(AddProviderForm, { fields: form, onChange: setForm, onSubmit: submitProvider }) : null
      ]
    }
  );
}
function ProviderCard({
  provider,
  defaultModel,
  confirmKey,
  tests,
  onSetDefault,
  onRemoveModel,
  onRemoveProvider,
  onTestRow,
  onConfirmCancel,
  deleteGuard
}) {
  const providerKey = `provider:${provider.id}`;
  const title = provider.name || provider.id;
  return /* @__PURE__ */ jsxs6(
    "div",
    {
      style: {
        border: "0.5px solid var(--ava-border-l2)",
        borderRadius: "var(--ava-radius-lg, 16px)",
        padding: "var(--ava-space-4, 16px)",
        background: "var(--ava-bg-layer-1)"
      },
      children: [
        /* @__PURE__ */ jsxs6("div", { style: { display: "flex", alignItems: "center", gap: "var(--ava-space-3, 10px)", marginBottom: "var(--ava-space-3, 10px)" }, children: [
          /* @__PURE__ */ jsx6("b", { style: { fontSize: 13 }, children: title }),
          /* @__PURE__ */ jsx6("span", { style: { fontSize: 11, color: "var(--ava-label-tertiary)", fontFamily: "var(--ava-font-mono, monospace)" }, children: provider.baseUrl }),
          /* @__PURE__ */ jsx6("span", { style: { flex: 1 } }),
          /* @__PURE__ */ jsxs6("span", { style: { position: "relative" }, children: [
            /* @__PURE__ */ jsx6("button", { style: BTN_DANGER, onClick: () => deleteGuard(providerKey, () => onRemoveProvider(provider.id)), children: "\u5220\u9664\u4F9B\u5E94\u5546" }),
            confirmKey === providerKey ? /* @__PURE__ */ jsx6(
              Popconfirm,
              {
                title: `\u5220\u9664\u4F9B\u5E94\u5546\u300C${title}\u300D\uFF1F\u7EC4\u5185\u9700\u65E0\u6A21\u578B\u65B9\u53EF\u5220\u9664\u3002`,
                onCancel: onConfirmCancel,
                onConfirm: () => onRemoveProvider(provider.id)
              }
            ) : null
          ] })
        ] }),
        /* @__PURE__ */ jsx6("table", { style: { borderCollapse: "collapse", width: "100%", fontSize: 12 }, children: /* @__PURE__ */ jsx6("tbody", { children: provider.models.map((model) => {
          const isDefault = model.id === defaultModel;
          const rowKey = `${provider.id}/${model.id}`;
          const modelKey = `model:${rowKey}`;
          const displayName = model.name && model.name !== model.id ? model.name : "";
          return /* @__PURE__ */ jsxs6("tr", { children: [
            /* @__PURE__ */ jsx6("td", { style: { padding: "5px 8px 5px 0", fontFamily: "var(--ava-font-mono, monospace)" }, children: model.id }),
            /* @__PURE__ */ jsx6("td", { style: { padding: "5px 8px", color: "var(--ava-label-secondary)" }, children: displayName }),
            /* @__PURE__ */ jsx6("td", { style: { padding: "5px 8px", color: "var(--ava-label-tertiary)" }, children: model.description ?? "" }),
            /* @__PURE__ */ jsx6("td", { style: { padding: "5px 8px", textAlign: "right" }, children: isDefault ? /* @__PURE__ */ jsx6("span", { style: CHIP(), children: "\u9ED8\u8BA4" }) : /* @__PURE__ */ jsx6("button", { style: BTN, onClick: () => onSetDefault(model.id), children: "\u8BBE\u9ED8\u8BA4" }) }),
            /* @__PURE__ */ jsx6("td", { style: { padding: "5px 8px", textAlign: "right" }, children: /* @__PURE__ */ jsx6(TestBadge, { state: tests[rowKey] }) }),
            /* @__PURE__ */ jsxs6("td", { style: { padding: "5px 0 5px 8px", textAlign: "right", whiteSpace: "nowrap" }, children: [
              /* @__PURE__ */ jsx6("button", { style: { ...BTN, marginRight: 6 }, onClick: () => onTestRow(provider.id, model.id), children: "\u6D4B\u8BD5" }),
              /* @__PURE__ */ jsxs6("span", { style: { position: "relative" }, children: [
                /* @__PURE__ */ jsx6("button", { style: BTN_DANGER, onClick: () => deleteGuard(modelKey, () => onRemoveModel(provider.id, model.id)), children: "\u5220\u9664" }),
                confirmKey === modelKey ? /* @__PURE__ */ jsx6(
                  Popconfirm,
                  {
                    title: `\u5220\u9664\u6A21\u578B\u300C${model.id}\u300D\uFF1F`,
                    onCancel: onConfirmCancel,
                    onConfirm: () => onRemoveModel(provider.id, model.id)
                  }
                ) : null
              ] })
            ] })
          ] }, model.id);
        }) }) })
      ]
    }
  );
}

// src/main.tsx
import { jsx as jsx8 } from "react/jsx-runtime";
function setup(ctx) {
  ctx.registerSettingsPanel(() => Promise.resolve().then(() => (init_settings_panel(), settings_panel_exports)));
  const disposers = [
    // lucide sliders-horizontal：模型/供应商配置语义
    ctx.slots.inject("nav.sidebar", {
      order: 7,
      viewId: "model-manager",
      title: "\u6A21\u578B\u7BA1\u7406",
      icon: "M21 4h-7 M10 4H3 M21 12h-9 M8 12H3 M21 20h-5 M12 20H3 M14 2v4 M8 10v4 M16 18v4"
    }),
    ctx.slots.inject("surface.main", {
      viewId: "model-manager",
      title: "\u6A21\u578B\u7BA1\u7406",
      component: async () => ({ default: () => /* @__PURE__ */ jsx8(ModelsView, { agent: ctx.agent }) })
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
