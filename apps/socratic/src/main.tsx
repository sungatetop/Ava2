// 苏格拉底实验室（Socratic）：真实 agent 追问引导 App。
// 用户旅程：选工作区 → 选题目文件（md/txt）→ 显示真实题目 → 开始追问（真实会话，
// agent 只提问不给答案）→ 学生作答 → agent 继续追问 → 画布沉淀真实推理板书 →
// 掌握度按真实交互轮次统计。全部内容来自 readWorkspaceFile 与真实 assistant 回复，无 mock。
// 结构：类型/枚举在 lib.ts，样式在 styles.ts，画布在 canvas.tsx，面板在 panels.tsx；
// 本文件承载主区交互（Main + awaitReply）与 slot 装配（setup）。
import type { AgentChannel, AppContext, FileEntry } from "@ava2/app-sdk";
import { useCallback, useEffect, useRef, useState } from "react";
import { Canvas } from "./canvas";
import { MENTOR_RULE, listQuestionFiles, listWorkdirs, textOf } from "./lib";
import type { Events, Session, StorageLike, Workdir } from "./lib";
import { CanvasPanel, SidebarPanel } from "./panels";
import { Styles } from "./styles";

/** 可重试动作：错误区「重试」按最近一次失败动作复放。 */
type Retry =
  | { kind: "init" }
  | { kind: "files"; w: Workdir }
  | { kind: "start"; f: FileEntry }
  | { kind: "ask"; sid: string; text: string; base: Session };

/** 主区：题目选择 + 真实追问对话。 */
function Main({ agent, events, storage }: { agent: AgentChannel; events: Events; storage: StorageLike }) {
  const [workdirs, setWorkdirs] = useState<Workdir[]>([]);
  const [workdir, setWorkdir] = useState<Workdir | null>(null);
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [sess, setSess] = useState<Session | null>(null);
  const [busy, setBusy] = useState(false);
  const [input, setInput] = useState("");
  const [error, setError] = useState<string | null>(null);
  /** 最近一次可重试动作（init / 枚举题目 / 开始追问 / 追问一轮）。 */
  const [retry, setRetry] = useState<Retry | null>(null);
  /** 递增触发 init 重跑（重试「无可用工作区」时复用挂载副作用）。 */
  const [initTick, setInitTick] = useState(0);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [sess]);

  const publish = useCallback(async (s: Session) => {
    setSess(s);
    await storage.set("session", s).catch(() => {});
    events.emit("socratic.turn", s);
  }, [storage, events]);

  const loadFiles = useCallback(async (w: Workdir) => {
    try {
      const list = await listQuestionFiles(agent, w);
      setFiles(list);
      setError(list.length === 0 ? "该工作区未找到 .md / .txt 题目文件。" : null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setFiles([]);
    }
  }, [agent]);

  const selectWorkdir = useCallback(async (w: Workdir) => {
    const apply = async () => {
      setWorkdir(w);
      setSess(null);
      // 记住上次工作区选择，init 时优先恢复。
      void storage.set("workdir", w.workingDir).catch(() => {});
      await loadFiles(w);
    };
    setRetry({ kind: "files", w });
    await apply();
  }, [loadFiles, storage]);

  const init = useCallback(async () => {
    setRetry({ kind: "init" });
    const list = await listWorkdirs(agent);
    setWorkdirs(list);
    if (list.length === 0) {
      setError("未找到会话工作区；请先在对话页打开一个会话。");
      return;
    }
    const remembered = await storage.get<string>("workdir").catch(() => undefined);
    const last = remembered ? list.find((w) => w.workingDir === remembered) : undefined;
    await selectWorkdir(last ?? list[0]);
  }, [agent, selectWorkdir, storage]);
  useEffect(() => { void init(); }, [init, initTick]);

  /** 等本轮 agent 回答收束后回读 durable 历史取最后一条 assistant 文本。 */
  const awaitReply = useCallback((sid: string): Promise<string> => new Promise((resolve) => {
    let settled = false;
    let d: { dispose(): void } | null = null;
    const tm = setTimeout(() => {
      if (settled) return;
      settled = true;
      if (d) d.dispose();
      resolve("");
    }, 90000);
    const finish = async () => {
      if (settled) return;
      settled = true;
      clearTimeout(tm);
      if (d) d.dispose();
      try {
        const frames = await agent.getMessages(sid);
        let last = "";
        for (const f of frames || []) {
          const e = f as unknown as Record<string, unknown>;
          if (e.type === "assistant_message") {
            const t = textOf(e.message);
            if (t.trim()) last = t;
          }
        }
        resolve(last);
      } catch {
        resolve("");
      }
    };
    d = agent.onSessionEvent(sid, (frame) => {
      const t = (frame as unknown as Record<string, unknown>).type;
      if (t === "turn_end" || t === "turn_report" || t === "session_closed") void finish();
    });
  }), [agent]);

  /** 向导师追问一轮：真实发送 → 等回复收束 → 追加导师轮次（作答与「重试」共用）。 */
  const askMentor = useCallback(async (sid: string, text: string, base: Session) => {
    setBusy(true);
    setError(null);
    try {
      await agent.send(sid, text);
      const reply = await awaitReply(sid);
      await publish({
        ...base,
        turns: [...base.turns, { role: "mentor", text: reply || "（未收到回复）", ts: Date.now() }],
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }, [agent, awaitReply, publish]);

  /** 开始追问：读题目 → 建会话 → 首轮（导师约束 + 题目）→ 记录真实回复。 */
  const start = useCallback(async (f: FileEntry) => {
    if (!workdir || busy) return;
    setBusy(true);
    setError(null);
    setRetry({ kind: "start", f });
    try {
      const res = await agent.readWorkspaceFile(f.path, workdir.sessionId);
      const qText = res.text ?? "";
      const created = await agent.createSession(workdir.workingDir);
      const first = `${MENTOR_RULE}\n\n【题目】\n${qText.slice(0, 2000)}\n\n请开始：先向学生提出第一个引导性问题。`;
      await agent.send(created.id, first);
      const reply = await awaitReply(created.id);
      const s: Session = {
        sessionId: created.id,
        questionPath: f.path,
        questionName: f.name,
        questionText: qText,
        turns: [{ role: "mentor", text: reply || "（未收到导师回复，请检查模型配置）", ts: Date.now() }],
      };
      await publish(s);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }, [agent, workdir, busy, awaitReply, publish]);

  /** 学生作答：真实发送 → 追加双方轮次。 */
  const answer = useCallback(async () => {
    const text = input.trim();
    if (!sess || !text || busy) return;
    setBusy(true);
    setInput("");
    const withStudent: Session = {
      ...sess,
      turns: [...sess.turns, { role: "student", text, ts: Date.now() }],
    };
    await publish(withStudent);
    setRetry({ kind: "ask", sid: sess.sessionId, text, base: withStudent });
    await askMentor(sess.sessionId, text, withStudent);
  }, [sess, input, busy, publish, askMentor]);

  /** 错误区「重试」：复放最近一次失败动作。 */
  const runRetry = useCallback(() => {
    if (!retry || busy) return;
    setError(null);
    switch (retry.kind) {
      case "init":
        setInitTick((t) => t + 1);
        break;
      case "files":
        void loadFiles(retry.w);
        break;
      case "start":
        void start(retry.f);
        break;
      case "ask":
        void askMentor(retry.sid, retry.text, retry.base);
        break;
    }
  }, [retry, busy, loadFiles, start, askMentor]);

  const mentorTurns = sess ? sess.turns.filter((t) => t.role === "mentor").length : 0;
  const studentTurns = sess ? sess.turns.filter((t) => t.role === "student").length : 0;

  return (
    <div className="sc-root">
      <Styles />
      <div className="sc-picker">
        <label>工作区</label>
        <select className="sc-select" value={workdir?.workingDir ?? ""} onChange={(e) => {
          const w = workdirs.find((x) => x.workingDir === e.target.value);
          if (w) void selectWorkdir(w);
        }}>
          {workdirs.length === 0 && <option value="">无可用工作区</option>}
          {workdirs.map((w) => <option key={w.workingDir} value={w.workingDir}>{w.workingDir}</option>)}
        </select>
        <label>题目</label>
        <select className="sc-select" value="" onChange={(e) => {
          const f = files.find((x) => x.path === e.target.value);
          if (f) void start(f);
        }} disabled={files.length === 0 || busy}>
          <option value="">{files.length === 0 ? "无题目文件" : `选择题目（${files.length}）…`}</option>
          {files.map((f) => <option key={f.path} value={f.path}>{f.name}</option>)}
        </select>
        <button className="sc-btn" onClick={() => {
          if (workdir) {
            setRetry({ kind: "files", w: workdir });
            void loadFiles(workdir);
          }
        }} disabled={!workdir || busy}>刷新</button>
        {sess && <span className="sc-meta">追问 {mentorTurns} 轮 · 作答 {studentTurns} 次</span>}
      </div>

      {error ? (
        <div style={{ padding: "12px 16px" }}>
          <div className="sc-err">
            {error}
            <div style={{ marginTop: 8 }}>
              <button className="sc-btn" onClick={runRetry} disabled={busy}>重试</button>
            </div>
          </div>
        </div>
      ) : null}

      <div className="sc-body" ref={scrollRef}>
        {busy && !sess ? (
          <div className="sc-empty"><span className="sc-spin" />正在建立真实会话并向导师提问…</div>
        ) : !sess ? (
          <div className="sc-empty">请选择工作区并挑选题目文件，系统会创建真实会话开始追问。</div>
        ) : (
          <>
            <div className="sc-question">
              <div className="hd">
                <span className="sc-chip">题目</span>
                <span className="sc-meta">{sess.questionName}</span>
              </div>
              <p>{sess.questionText.slice(0, 1200)}</p>
            </div>
            <div className="sc-dialogue">
              {sess.turns.map((t, i) => (
                <div className={"sc-msg" + (t.role === "student" ? " me" : "")} key={i}>
                  <div className={"sc-ava" + (t.role === "student" ? " student" : "")}>{t.role === "student" ? "生" : "苏"}</div>
                  <div className="sc-bubble">
                    {t.text}
                    <span className="sc-why">{t.role === "student" ? "学生自述" : `导师追问 · 第 ${sess.turns.slice(0, i + 1).filter((x) => x.role === "mentor").length} 轮`}</span>
                  </div>
                </div>
              ))}
              {busy && sess && <div className="sc-empty"><span className="sc-spin" />导师正在思考下一个问题…</div>}
            </div>
          </>
        )}
      </div>

      <div className="sc-inputbar">
        <input
          value={input}
          placeholder={sess ? "以学生口吻作答，导师会继续追问（不给答案）" : "请先选择题目"}
          disabled={!sess || busy}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && answer()}
        />
        <button className="sc-send" onClick={answer} disabled={!sess || busy || !input.trim()} aria-label="发送">↑</button>
      </div>
    </div>
  );
}

export function setup(ctx: AppContext): { dispose(): void } {
  const disposers = [
    ctx.slots.inject("nav.sidebar", {
      order: 9, viewId: "socratic", title: "苏格拉底实验室",
      icon: "M14 2a1 1 0 0 1 1 1v2.1c1.8.3 3 1.9 3 3.9s-1.2 3.6-3 3.9V21a1 1 0 0 1-2 0v-8.1c-1.8-.3-3-1.9-3-3.9s1.2-3.6 3-3.9V3a1 1 0 0 1 1-1zm0 8a2 2 0 1 0 0-4 2 2 0 0 0 0 4zM5 8a1 1 0 0 1 1 1 1 1 0 0 1-1 1H4a1 1 0 0 1 0-2h1z",
    }),
    ctx.slots.inject("surface.main", {
      viewId: "socratic", title: "苏格拉底实验室",
      component: async () => ({ default: () => <Main agent={ctx.agent} events={ctx.events} storage={ctx.storage} /> }),
    }),
    ctx.slots.inject("surface.canvas", {
      viewId: "socratic-board", title: "解题板书", description: "真实追问序列与作答记录",
      component: async () => ({ default: () => <Canvas agent={ctx.agent} events={ctx.events} storage={ctx.storage} /> }),
    }),
    ctx.slots.inject("panel.sidebar", {
      order: 9, viewId: "socratic", title: "交互统计",
      component: async () => ({ default: () => <SidebarPanel agent={ctx.agent} events={ctx.events} storage={ctx.storage} /> }),
    }),
    ctx.slots.inject("panel.canvas", {
      order: 9, viewId: "socratic-board", title: "会话信息",
      component: async () => ({ default: () => <CanvasPanel agent={ctx.agent} events={ctx.events} storage={ctx.storage} /> }),
    }),
  ];
  return { dispose() { for (const d of disposers) d.dispose(); } };
}
