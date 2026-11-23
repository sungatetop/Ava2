// 会话工作台（第二个真实 App，roadmap 3.1 狗粮验证）：裸 ESM + import map 供给 react。
// 契约消费矩阵（docs/solutions/sessionboard-app.md §2）：五类 slot 全量 + 五类 inject 面。
// - surface.main `board`：会话仪表盘（统计 + 列表 + 新建/删除/试验）；
// - surface.canvas `live`：对话试验台（选会话 → send → onSessionEvent 流式往返）；
// - panel.sidebar / panel.canvas：双面板（各自独立拉数据，验证面板 slot 契约）；
// - storage 记忆选中会话、events emit canvas.open 跨视图编排；
// - 配置自管（宿主零策略）：pageSize/confirmDelete 存 localStorage，经设置面板（registerSettingsPanel）修改。
import * as React from "react";

const { createElement: h, useCallback, useEffect, useLayoutEffect, useRef, useState } = React;

// ---- 帧形状助手（对齐 chatapp frame-reducer 的集中断言风格） ----

function asRec(value) {
  return typeof value === "object" && value !== null ? value : undefined;
}

function str(value) {
  return typeof value === "string" ? value : "";
}

/** message.blocks[] 中 text 块拼接 */
function blocksText(message) {
  const m = asRec(message);
  const blocks = Array.isArray(m?.blocks) ? m.blocks : [];
  return blocks.map((b) => (asRec(b)?.type === "text" ? str(asRec(b)?.text) : "")).join("");
}

function fmtTime(ms) {
  if (typeof ms !== "number" || ms <= 0) return "—";
  return new Date(ms).toLocaleString();
}

// ---- 试验台 transcript reducer：帧流 → { msgs, busy, retry } ----
// 只消费文本往返（user_message / assistant_message / AssistantStream.Text / turn 收束），
// 工具行与待回填帧由 chatapp 承担，此处静默忽略。

function appendAssistant(state, text, replace) {
  const msgs = state.msgs;
  const last = msgs[msgs.length - 1];
  if (last && last.role === "assistant" && last.streaming) {
    const next = msgs.slice();
    next[msgs.length - 1] = { role: "assistant", text: replace ? text : last.text + text, streaming: true };
    return { ...state, msgs: next };
  }
  return { ...state, msgs: [...msgs, { role: "assistant", text, streaming: true }] };
}

function finalize(state) {
  const msgs = state.msgs;
  const last = msgs[msgs.length - 1];
  const next =
    last && last.role === "assistant" && last.streaming
      ? [...msgs.slice(0, -1), { role: "assistant", text: last.text || "（本轮无文本回复）", streaming: false }]
      : msgs;
  return { ...state, msgs: next, busy: false };
}

function applyDurable(state, raw) {
  const ev = asRec(raw);
  if (!ev) return state;
  switch (ev.type) {
    case "turn_start":
      return { ...state, busy: true };
    case "user_message": {
      // source 非空 = 宿主注入帧（baseline/env 等），不渲染为对话
      if (asRec(ev.message)?.source != null) return state;
      const text = blocksText(ev.message);
      if (!text) return state;
      // 乐观气泡对账：从后向前找同文本乐观项就地转正
      const msgs = state.msgs;
      for (let i = msgs.length - 1; i >= 0; i--) {
        const it = msgs[i];
        if (it.role === "user" && it.optimistic && it.text === text) {
          const next = msgs.slice();
          next[i] = { role: "user", text };
          return { ...state, msgs: next };
        }
      }
      return { ...state, msgs: [...msgs, { role: "user", text }] };
    }
    case "assistant_message": {
      const text = blocksText(ev.message);
      if (!text) return state;
      return appendAssistant(state, text, true); // durable 全量替换流式缓冲
    }
    case "assistant_attempt": {
      const error = str(ev.error);
      return error ? { ...state, retry: error } : state;
    }
    case "turn_end":
      return finalize(state);
    default:
      return state;
  }
}

function applyFrame(state, frame) {
  const f = asRec(frame);
  if (!f) return state;
  switch (f.type) {
    case "snapshot": {
      const events = Array.isArray(f.events) ? f.events : [];
      return events.reduce(applyDurable, state);
    }
    case "durable":
      return applyDurable(state, f.event);
    case "live": {
      const ev = asRec(f.event);
      if (!ev) return state;
      if (ev.Durable !== undefined || ev.durable !== undefined) return state; // Durable 包装由 durable 帧承载
      const stream = asRec(ev.AssistantStream ?? ev.assistant_stream);
      const text = stream && typeof stream.Text === "string" ? stream.Text : "";
      if (!text) return state;
      return appendAssistant(state, text, false); // live 增量追加
    }
    case "turn_report":
      return finalize(state);
    default:
      return state;
  }
}

const EMPTY_CHAT = { msgs: [], busy: false, retry: "" };

// ---- 气泡确认（Popconfirm）：锚定触发点、点外取消、无遮罩（对齐浮层控件指南）----
// fixed 定位按锚点 rect 计算，避开滚动容器 overflow 裁剪；clamp 视口，放不下向上弹。

const PC_BTN = { height: 26, padding: "0 12px", border: "0.5px solid rgba(128,128,128,.35)", borderRadius: 8, background: "transparent", fontSize: 12, cursor: "pointer" };

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
  return h(
    "div",
    {
      ref,
      role: "dialog",
      "aria-label": "确认操作",
      onClick: (e) => e.stopPropagation(),
      style: {
        position: "fixed",
        zIndex: 40,
        minWidth: 224,
        maxWidth: 268,
        padding: 10,
        border: "0.5px solid rgba(128,128,128,.35)",
        borderRadius: 10,
        background: "var(--ava-surface-elevated, #fff)",
        boxShadow: "0 8px 24px rgba(0,0,0,.14)",
        fontSize: 12,
        color: "var(--ava-text-primary, #1a1a1a)",
        ...(pos ? { top: pos.top, left: pos.left } : { visibility: "hidden" }),
      },
    },
    h("div", { style: { lineHeight: 1.6, wordBreak: "break-word" } }, title),
    h(
      "div",
      { style: { display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 8 } },
      h("button", { style: { ...PC_BTN, color: "var(--ava-text-secondary, #555)" }, onClick: onCancel }, "取消"),
      h(
        "button",
        { style: { ...PC_BTN, borderColor: "#ec1313", color: "#ec1313" }, onClick: () => { onCancel(); onConfirm(); } },
        "确认",
      ),
    ),
  );
}

// ---- 主区：会话仪表盘（surface.main: board） ----

function Board(ctx) {
  return function BoardView() {
    const pageSize = readCfg("pageSize", 8);
    const confirmDelete = readCfg("confirmDelete", true);
    const [sessions, setSessions] = useState(null);
    const [err, setErr] = useState("");
    const [note, setNote] = useState("");
    const [busyId, setBusyId] = useState("");
    const [confirmId, setConfirmId] = useState(""); // 气泡确认中的会话 id

    const load = useCallback(() => {
      setErr("");
      ctx.agent
        .listSessions()
        .then((r) => setSessions(Array.isArray(r) ? r : []))
        .catch((e) => setErr(e.message || String(e)));
    }, []); // eslint-disable-line react-hooks/exhaustive-deps

    useEffect(() => {
      load();
    }, [load]);

    const create = async () => {
      setErr("");
      setNote("");
      setBusyId("__create");
      try {
        const s = await ctx.agent.createSession();
        setNote(`已新建会话 ${s.id}`);
        load();
      } catch (e) {
        setErr(e.message || String(e));
      } finally {
        setBusyId("");
      }
    };

    const remove = async (id) => {
      setErr("");
      setNote("");
      setBusyId(id);
      try {
        await ctx.agent.deleteSession(id); // 404 幂等
        setNote(`已删除 ${id}`);
        load();
      } catch (e) {
        setErr(e.message || String(e));
      } finally {
        setBusyId("");
      }
    };

    /** 「试验」：storage 预选 + 总线 canvas.open 打开画布试验台（跨视图编排） */
    const openLive = (id) => {
      void ctx.storage.set("lastSessionId", id);
      ctx.events.emit("canvas.open", { viewId: "live" });
    };

    const list = sessions ?? [];
    const shown = list.slice(0, Math.max(1, pageSize));
    const liveCount = list.filter((s) => s.live).length;

    return h(
      "div",
      { className: "sb-root", style: { padding: "var(--ava-space-4, 16px)", fontSize: 13, lineHeight: 1.6 } },
      h("h2", { style: { margin: "0 0 var(--ava-space-3, 10px)" } }, "会话工作台"),
      h(
        "div",
        { style: { display: "flex", gap: "var(--ava-space-4, 16px)", alignItems: "center", marginBottom: "var(--ava-space-3, 10px)" } },
        h("span", null, "总数 ", h("b", null, String(list.length))),
        h("span", null, "live ", h("b", null, String(liveCount))),
        h("span", null, "空闲 ", h("b", null, String(list.length - liveCount))),
        h("button", { style: { marginLeft: "auto" }, onClick: () => load() }, "刷新"),
        h("button", { onClick: () => void create(), disabled: busyId !== "" }, "新建会话"),
      ),
      err && h("p", { style: { color: "#ec1313", margin: "0 0 var(--ava-space-2, 8px)" } }, "操作失败：", err),
      note && h("p", { style: { opacity: 0.7, margin: "0 0 var(--ava-space-2, 8px)" } }, note),
      sessions === null && !err && h("p", { style: { opacity: 0.6 } }, "加载中…"),
      sessions !== null && list.length === 0 && h("p", { style: { opacity: 0.6 } }, "暂无会话，点击「新建会话」开始"),
      list.length > 0 &&
        h(
          "table",
          { style: { width: "100%", borderCollapse: "collapse" } },
          h(
            "thead",
            null,
            h(
              "tr",
              null,
              ["标题", "会话 id", "事件数", "创建时间", "操作"].map((t) =>
                h("th", { key: t, style: { textAlign: "left", opacity: 0.6, fontWeight: 500, padding: "var(--ava-space-1, 4px) var(--ava-space-2, 8px)", borderBottom: "1px solid rgba(128,128,128,.3)" } }, t),
              ),
            ),
          ),
          h(
            "tbody",
            null,
            shown.map((s) =>
              h(
                "tr",
                { key: s.id },
                h("td", { style: { padding: "var(--ava-space-1, 4px) var(--ava-space-2, 8px)" } }, s.title || "（未命名）", s.live ? h("span", { style: { marginLeft: "var(--ava-space-2, 8px)", fontSize: 11, opacity: 0.7 } }, "● live") : null),
                h("td", { style: { padding: "var(--ava-space-1, 4px) var(--ava-space-2, 8px)", fontFamily: "var(--ava-font-mono, monospace)", fontSize: 12, opacity: 0.8 } }, s.id),
                h("td", { style: { padding: "var(--ava-space-1, 4px) var(--ava-space-2, 8px)" } }, String(s.eventCount ?? "—")),
                h("td", { style: { padding: "var(--ava-space-1, 4px) var(--ava-space-2, 8px)", opacity: 0.8 } }, fmtTime(s.createdAtMs)),
                h(
                  "td",
                  { style: { padding: "var(--ava-space-1, 4px) var(--ava-space-2, 8px)" } },
                  h("button", { onClick: () => openLive(s.id), title: "在画布试验台打开该会话" }, "试验"),
                  h(
                    "button",
                    { style: { marginLeft: "var(--ava-space-2, 8px)" }, onClick: () => (confirmDelete ? setConfirmId(s.id) : void remove(s.id)), disabled: busyId === s.id },
                    "删除",
                  ),
                  confirmId === s.id &&
                    h(
                      "span",
                      { style: { display: "inline-block" } },
                      h(Popconfirm, {
                        title: `删除会话 ${s.id}？历史记录将从磁盘移除，不可恢复。`,
                        onCancel: () => setConfirmId(""),
                        onConfirm: () => {
                          setConfirmId("");
                          void remove(s.id);
                        },
                      }),
                    ),
                ),
              ),
            ),
          ),
        ),
      list.length > shown.length && h("p", { style: { opacity: 0.6 } }, `（其余 ${list.length - shown.length} 条未显示，可在设置中调大 pageSize）`),
    );
  };
}

// ---- 侧栏面板：会话速览（panel.sidebar: board） ----

function BoardPanel(ctx) {
  return function BoardPanelView() {
    const [sessions, setSessions] = useState(null);
    useEffect(() => {
      let alive = true;
      ctx.agent
        .listSessions()
        .then((r) => alive && setSessions(Array.isArray(r) ? r : []))
        .catch(() => alive && setSessions([]));
      return () => {
        alive = false;
      };
    }, []);
    const list = sessions ?? [];
    const latest = list[0];
    return h(
      "div",
      { style: { fontSize: 12, lineHeight: 1.7 } },
      h("div", null, "会话总数：", h("b", null, sessions === null ? "…" : String(list.length))),
      latest
        ? h("div", { style: { opacity: 0.8 } }, "最近：", latest.title || latest.id)
        : h("div", { style: { opacity: 0.6 } }, "暂无会话"),
    );
  };
}

// ---- 画布：对话试验台（surface.canvas: live） ----

function Live(ctx) {
  return function LiveView() {
    const [sessions, setSessions] = useState([]);
    const [sel, setSel] = useState("");
    const [chat, setChat] = useState(EMPTY_CHAT);
    const [gotSnapshot, setGotSnapshot] = useState(false);
    const [input, setInput] = useState("");
    const [err, setErr] = useState("");
    const [listErr, setListErr] = useState("");

    // 会话清单（供选择器）
    useEffect(() => {
      let alive = true;
      ctx.agent
        .listSessions()
        .then((r) => alive && setSessions(Array.isArray(r) ? r : []))
        .catch((e) => alive && setListErr(e.message || String(e)));
      return () => {
        alive = false;
      };
    }, []);

    // 恢复上次选中（storage）；无记录则选第一个
    useEffect(() => {
      if (sel || sessions.length === 0) return;
      let cancelled = false;
      ctx.storage.get("lastSessionId").then((saved) => {
        if (cancelled) return;
        const target = saved && sessions.some((s) => s.id === saved) ? saved : sessions[0].id;
        setSel(target);
      });
      return () => {
        cancelled = true;
      };
    }, [sessions, sel]);

    // 选中持久化 + 广播（panel.canvas 跟随同一会话）
    useEffect(() => {
      if (!sel) return;
      void ctx.storage.set("lastSessionId", sel);
      ctx.events.emit("sessionboard.select", { id: sel });
    }, [sel]);

    // 订阅选中会话事件流：首个 snapshot 帧构建历史（与 chatapp 同构，不用 getMessages）
    useEffect(() => {
      if (!sel) return;
      setChat(EMPTY_CHAT);
      setGotSnapshot(false);
      setErr("");
      const off = ctx.agent.onSessionEvent(sel, (frame) => {
        if (asRec(frame)?.type === "snapshot") setGotSnapshot(true);
        setChat((prev) => applyFrame(prev, frame));
      });
      return () => off.dispose();
    }, [sel]);

    const createAndSelect = async () => {
      setListErr("");
      try {
        const s = await ctx.agent.createSession();
        setSessions((cur) => [{ id: s.id, title: s.id, live: true, eventCount: 0 }, ...cur]);
        setSel(s.id);
      } catch (e) {
        setListErr(e.message || String(e));
      }
    };

    const send = async () => {
      const text = input.trim();
      if (!text || !sel || chat.busy) return;
      setInput("");
      setChat((prev) => ({ ...prev, msgs: [...prev.msgs, { role: "user", text, optimistic: true }], busy: true, retry: "" }));
      try {
        await ctx.agent.send(sel, text); // wait=false：进度走事件流
      } catch (e) {
        setChat((prev) => ({ ...prev, busy: false, retry: e.message || String(e) }));
      }
    };

    const busy = chat.busy;
    const inputEl = h("textarea", {
      value: input,
      rows: 2,
      placeholder: busy ? "助手回复中…" : "输入消息（Enter 发送，Shift+Enter 换行）",
      style: { flex: 1, resize: "vertical", fontSize: 13, padding: "var(--ava-space-2, 8px) var(--ava-space-2, 8px)" },
      onChange: (e) => setInput(e.target.value),
      onKeyDown: (e) => {
        if (e.key === "Enter" && !e.shiftKey) {
          e.preventDefault();
          void send();
        }
      },
    });

    return h(
      "div",
      { className: "sb-root", style: { padding: "var(--ava-space-3, 10px)", fontSize: 13, display: "flex", flexDirection: "column", height: "100%", boxSizing: "border-box", gap: "var(--ava-space-2, 8px)" } },
      h(
        "div",
        { style: { display: "flex", gap: "var(--ava-space-2, 8px)", alignItems: "center" } },
        h(
          "select",
          {
            value: sel,
            style: { flex: 1, maxWidth: 420, fontSize: 13 },
            onChange: (e) => setSel(e.target.value),
          },
          sessions.length === 0 && h("option", { value: "" }, "（暂无会话）"),
          sessions.map((s) => h("option", { key: s.id, value: s.id }, `${s.title || s.id}（${s.id.slice(0, 8)}…）`)),
        ),
        h("button", { onClick: () => void createAndSelect() }, "新建"),
        h("span", { style: { opacity: 0.7, fontSize: 12 } }, busy ? "● 回复中" : gotSnapshot ? "○ 空闲" : "… 连接"),
      ),
      listErr && h("p", { style: { color: "#ec1313", margin: 0, fontSize: 12 } }, listErr),
      !sel && h("p", { style: { opacity: 0.6, margin: 0 } }, "选择或新建一个会话后开始对话"),
      sel && !gotSnapshot && h("p", { style: { opacity: 0.6, margin: 0 } }, "事件流连接中…"),
      h(
        "div",
        { style: { flex: 1, overflowY: "auto", border: "1px solid rgba(128,128,128,.25)", borderRadius: "var(--ava-radius-sm, 10px)", padding: "var(--ava-space-2, 8px) var(--ava-space-3, 10px)", minHeight: 120 } },
        chat.msgs.length === 0 && gotSnapshot && h("p", { style: { opacity: 0.6, margin: 0 } }, "（空会话，发送第一条消息）"),
        chat.msgs.map((m, i) =>
          h(
            "div",
            { key: i, style: { margin: "0 0 var(--ava-space-2, 8px)" } },
            h("div", { style: { fontSize: 11, opacity: 0.55 } }, m.role === "user" ? "你" : "助手", m.optimistic ? "（发送中）" : m.streaming ? "（流式）" : ""),
            h("div", { style: { whiteSpace: "pre-wrap", opacity: m.optimistic ? 0.7 : 1 } }, m.text),
          ),
        ),
      ),
      chat.retry && h("p", { style: { color: "#ec1313", margin: 0, fontSize: 12 } }, "重试提示：", chat.retry),
      h(
        "div",
        { style: { display: "flex", gap: "var(--ava-space-2, 8px)", alignItems: "flex-end" } },
        inputEl,
        h("button", { onClick: () => void send(), disabled: busy || !sel || input.trim() === "" }, "发送"),
      ),
    );
  };
}

// ---- 画布面板：运行状态（panel.canvas: live） ----

function LivePanel(ctx) {
  return function LivePanelView() {
    const [model, setModel] = useState("查询中…");
    const [sel, setSel] = useState("");
    const [busy, setBusy] = useState(null); // null = 未连接
    const [report, setReport] = useState(null);
    const [exists, setExists] = useState(true);

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

    // 跟随 storage 中的选中会话：独立订阅同一 WS（验证多路复用引用计数）
    useEffect(() => {
      let cancelled = false;
      let currentOff = null;
      const watch = async (id) => {
        if (currentOff) {
          currentOff.dispose();
          currentOff = null;
        }
        setSel(id);
        setBusy(null);
        setReport(null);
        setExists(true);
        if (!id) return;
        try {
          await ctx.agent.getSession(id);
        } catch {
          if (!cancelled) setExists(false);
          return;
        }
        if (cancelled) return;
        currentOff = ctx.agent.onSessionEvent(id, (frame) => {
          const f = asRec(frame);
          if (!f) return;
          if (f.type === "snapshot") setBusy(false); // 快照到达 = 连接建立且无进行中轮次
          else if (f.type === "durable") {
            const ev = asRec(f.event);
            if (ev?.type === "turn_start") setBusy(true);
            if (ev?.type === "turn_end") setBusy(false);
          } else if (f.type === "live") {
            if (asRec(f.event)?.AssistantStream !== undefined) setBusy(true);
          } else if (f.type === "turn_report") {
            setBusy(false);
            setReport({ turn: f.turn, steps: f.steps, reason: str(f.reason) });
          }
        });
      };
      ctx.storage.get("lastSessionId").then((saved) => {
        if (!cancelled) void watch(saved || "");
      });
      const onChange = (e) => {
        const id = asRec(e)?.id;
        if (typeof id === "string" && id) void watch(id);
      };
      const offBus = ctx.events.on("sessionboard.select", onChange);
      return () => {
        cancelled = true;
        if (currentOff) currentOff.dispose();
        offBus.dispose();
      };
    }, []);

    return h(
      "div",
      { style: { fontSize: 12, lineHeight: 1.7 } },
      h("div", null, h("b", null, "运行状态："), "默认模型 ", model),
      !sel && h("div", { style: { opacity: 0.6 } }, "未选中会话（在试验台选择后跟随）"),
      sel && !exists && h("div", { style: { opacity: 0.6 } }, `会话 ${sel.slice(0, 8)}… 已不存在`),
      sel && exists && h("div", null, "会话 ", h("code", { style: { fontSize: 11 } }, sel), "：", busy === null ? "连接中" : busy ? "回复中" : "空闲"),
      report && h("div", { style: { opacity: 0.8 } }, `最近轮次 #${report.turn}：${report.steps} 步（${report.reason || "未知"}）`),
    );
  };
}

// ---- 设置面板（宿主只提供入口：ctx.registerSettingsPanel；表单/存储/校验由本 App 自理） ----

const CFG_KEY = "sessionboard:cfg";

function readCfg(key, fallback) {
  try {
    const raw = JSON.parse(localStorage.getItem(CFG_KEY) || "{}");
    const v = raw[key];
    return v === undefined ? fallback : v;
  } catch {
    return fallback;
  }
}

function writeCfg(patch) {
  let raw = {};
  try {
    raw = JSON.parse(localStorage.getItem(CFG_KEY) || "{}");
  } catch {
    /* 脏值重置 */
  }
  localStorage.setItem(CFG_KEY, JSON.stringify({ ...raw, ...patch }));
}

function SettingsPanel() {
  const [pageSize, setPageSize] = useState(() => readCfg("pageSize", 8));
  const [confirmDelete, setConfirmDelete] = useState(() => readCfg("confirmDelete", true));
  const inputStyle = { padding: "4px 8px", border: "0.5px solid var(--ava-border-l2)", borderRadius: 6, background: "var(--ava-bg-base)", color: "var(--ava-label-primary)", font: "inherit", width: 90 };
  return h(
    "div",
    { style: { display: "flex", flexDirection: "column", gap: 12, padding: "4px 0", fontSize: 13 } },
    h(
      "label",
      { style: { display: "flex", alignItems: "center", gap: 8 } },
      "列表条数上限",
      h("input", {
        type: "number",
        min: 1,
        max: 50,
        value: pageSize,
        style: inputStyle,
        onChange: (e) => {
          const n = Number(e.target.value);
          // 本 App 自行校验（1-50 夹取），宿主不参与
          const v = Number.isFinite(n) ? Math.min(50, Math.max(1, Math.round(n))) : 8;
          setPageSize(v);
          writeCfg({ pageSize: v });
        },
      }),
    ),
    h(
      "label",
      { style: { display: "flex", alignItems: "center", gap: 8, cursor: "pointer" } },
      h("input", {
        type: "checkbox",
        checked: confirmDelete,
        onChange: (e) => {
          setConfirmDelete(e.target.checked);
          writeCfg({ confirmDelete: e.target.checked });
        },
      }),
      "删除前弹出气泡确认",
    ),
    h("div", { style: { fontSize: 12, color: "var(--ava-label-tertiary)" } }, "配置由本 App 自行存储（localStorage），即时生效。"),
  );
}

export function setup(ctx) {
  // 登记设置面板：应用市场详情页出现「设置」入口（懒挂载，宿主不参与存储/校验）
  ctx.registerSettingsPanel(async () => ({ default: SettingsPanel }));
  const disposers = [
    ctx.slots.inject("nav.sidebar", {
      order: 6,
      viewId: "board",
      title: "会话工作台",
      icon: "M3 5h18v14H3z M3 10h18 M10 10v9",
    }),
    ctx.slots.inject("surface.main", { viewId: "board", title: "会话工作台", component: async () => ({ default: Board(ctx) }) }),
    ctx.slots.inject("panel.sidebar", { order: 6, viewId: "board", title: "会话速览", component: async () => ({ default: BoardPanel(ctx) }) }),
    ctx.slots.inject("surface.canvas", {
      viewId: "live",
      title: "对话试验台",
      description: "选中会话并发起真实对话往返（agent 通道）",
      component: async () => ({ default: Live(ctx) }),
    }),
    ctx.slots.inject("panel.canvas", { order: 6, viewId: "live", title: "运行状态", component: async () => ({ default: LivePanel(ctx) }) }),
  ];
  // 总线自检：观察宿主广播（卸载时自动排空）
  disposers.push(ctx.events.on("ping", (e) => console.info("[sessionboard] ping", e)));

  return {
    dispose() {
      disposers.forEach((d) => d.dispose());
    },
  };
}
