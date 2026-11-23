// 对话流视图：借鉴原型，管家发言用文档式 prose，用户发言右对齐色块；
// 派发卡 / 运行态活动条 / 升级卡 / 失败卡 / 执行结果卡承载结构化信息；
// 底部为胶囊输入（textarea 自适应 + 发送）。
import { useEffect, useRef, useState } from "react";
import type { ButlerNotice, ButlerSnapshot, ButlerWorker, Discipline, Escalation } from "./engine";
import { useButler, type ButlerEngine } from "./engine";
import { Styles } from "./styles";

/** 相对时间：刚刚 / N 分钟前 / HH:MM。 */
export function relTime(ts: number): string {
  const diff = Date.now() - ts;
  if (diff < 60_000) return "刚刚";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)} 分钟前`;
  const d = new Date(ts);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

/** 计时：mm:ss。 */
function elapsed(from: number): string {
  const s = Math.max(0, Math.floor((Date.now() - from) / 1000));
  return `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
}

/** 极简 markdown-ish 渲染：段落 / 列表 / 行内 code 与加粗。 */
function Prose({ text }: { text: string }) {
  const blocks: React.ReactNode[] = [];
  const lines = text.split(/\r?\n/);
  let para: string[] = [];
  let list: string[] = [];
  let ordered = false;
  const flushPara = () => {
    if (para.length) {
      blocks.push(<p key={`p${blocks.length}`}>{inline(para.join(" "))}</p>);
      para = [];
    }
  };
  const flushList = () => {
    if (list.length) {
      const items = list.map((t, i) => <li key={i}>{inline(t)}</li>);
      blocks.push(ordered ? <ol key={`l${blocks.length}`}>{items}</ol> : <ul key={`l${blocks.length}`}>{items}</ul>);
      list = [];
    }
  };
  for (const raw of lines) {
    const line = raw.trimEnd();
    const ul = /^\s*[-*•]\s+(.*)$/.exec(line);
    const ol = /^\s*\d+[.)]\s+(.*)$/.exec(line);
    if (ul || ol) {
      flushPara();
      const isOrdered = Boolean(ol);
      if (list.length && ordered !== isOrdered) flushList();
      ordered = isOrdered;
      list.push((ul ? ul[1] : ol![1]).trim());
      continue;
    }
    if (!line.trim()) {
      flushPara();
      flushList();
      continue;
    }
    flushList();
    para.push(line);
  }
  flushPara();
  flushList();
  return <div className="bt-prose">{blocks.length ? blocks : <p>{inline(text)}</p>}</div>;
}

/** 行内 `code` 与 **bold**。 */
function inline(s: string): React.ReactNode[] {
  const out: React.ReactNode[] = [];
  const re = /`([^`]+)`|\*\*([^*]+)\*\*/g;
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(s)) !== null) {
    if (m.index > last) out.push(s.slice(last, m.index));
    if (m[1] !== undefined) out.push(<code key={m.index}>{m[1]}</code>);
    else out.push(<b key={m.index}>{m[2]}</b>);
    last = m.index + m[0].length;
  }
  if (last < s.length) out.push(s.slice(last));
  return out;
}

/** 执行结果卡：默认折叠长正文。 */
function ResultCard({ text, body, workerId }: { text: string; body: string; workerId?: string }) {
  const long = body.length > 220;
  const [open, setOpen] = useState(!long);
  return (
    <div className={"bt-result" + (open ? "" : " collapsed")}>
      <div className="rh" onClick={() => setOpen((v) => !v)}>
        <span className="tick">✓</span>
        <span className="name">{text}</span>
        {workerId ? <span className="bt-chip">{workerId.slice(0, 8)}</span> : null}
        {long ? <span className="fold">{open ? "收起" : "展开全文"}</span> : null}
      </div>
      <div className="rbody"><Prose text={body} /></div>
    </div>
  );
}

function Notice({ n, onOpenWorker }: { n: ButlerNotice; onOpenWorker?: (id: string) => void }) {
  if (n.kind === "delegate") {
    const w = n.workerId ?? "";
    const open = w && onOpenWorker ? () => onOpenWorker(w) : undefined;
    return (
      <div
        className={"bt-dispatch" + (open ? " clickable" : "")}
        role={open ? "button" : undefined}
        tabIndex={open ? 0 : undefined}
        title={open ? "打开该会话" : undefined}
        onClick={open}
        onKeyDown={(e) => {
          if (open && (e.key === "Enter" || e.key === " ")) {
            e.preventDefault();
            open();
          }
        }}
      >
        <span className="glyph">D</span>
        <span className="body">
          <span className="name">{n.text}</span>
          <span className="meta">
            <span className="bt-chip ok">已派发</span>
            <span>会话 {w.slice(0, 8)} · {relTime(n.ts)}</span>
          </span>
        </span>
        {open ? <span className="arrow">›</span> : null}
      </div>
    );
  }
  if (n.kind === "report") {
    return <ResultCard text={n.text} body={n.body ?? ""} workerId={n.workerId} />;
  }
  if (n.kind === "morning") {
    return (
      <div className="bt-morning">
        <div className="hd"><span className="sun">晨</span>晨报 · 今日概况</div>
        <ul>{(n.summary || []).map((s, i) => <li key={i}>{s}</li>)}</ul>
      </div>
    );
  }
  if (n.kind === "failure") {
    return <FailureCard notice={n} />;
  }
  if (n.role === "user") {
    return <div className="bt-self"><div className="bubble">{n.text}</div></div>;
  }
  return <Prose text={n.text} />;
}

function FailureCard({ notice }: { notice: ButlerNotice }) {
  const { retry } = useButlerEngineSafe();
  return (
    <div className="bt-failure">
      <div className="head">{notice.text}</div>
      {notice.body ? <div className="detail">{notice.body}</div> : null}
      <div className="acts">
        {notice.retryText && retry ? (
          <button className="primary" onClick={() => retry(notice.retryText!, notice.id)}>重试</button>
        ) : null}
        <button onClick={() => void navigator.clipboard?.writeText(`${notice.text}\n${notice.body ?? ""}`)}>复制错误</button>
      </div>
    </div>
  );
}

/** 供深层卡片取 engine 的轻量上下文（避免层层传 props）。 */
let currentEngine: ButlerEngine | null = null;
function useButlerEngineSafe(): { retry: ((t: string, failureId?: string) => void) | null } {
  if (!currentEngine) return { retry: null };
  const engine = currentEngine;
  return { retry: (t: string, failureId?: string) => void engine.retry(t, failureId) };
}

function EscalationCard({ e, onResolve }: { e: Escalation; onResolve: (d: "approve" | "deny") => void }) {
  const isPlan = e.item.kind === "plan_review";
  return (
    <div className="bt-escalation">
      <div className="head">
        <span className="tag">L4</span>
        <span>需要你处理</span>
      </div>
      <div className="body">
        任务「{e.sessionTitle}」请求执行 <code>{e.item.tool || e.item.kind}</code>
        {e.item.reason === "sandbox-retry" ? "，已被硬护栏拦截" : ""}，需你确认。
      </div>
      <div className="acts">
        <button className="go" onClick={() => onResolve("approve")}>{isPlan ? "通过计划" : "批准"}</button>
        <button onClick={() => onResolve("deny")}>{isPlan ? "继续规划" : "拒绝"}</button>
      </div>
    </div>
  );
}

/** 运行态活动条。 */
function Activity({ phase, from }: { phase: string; from: number }) {
  const [, tick] = useState(0);
  useEffect(() => {
    const t = setInterval(() => tick((v) => v + 1), 1000);
    return () => clearInterval(t);
  }, []);
  return (
    <div className="bt-activity">
      <span className="spin" />
      <span className="phase">{phase || "正在处理"}</span>
      <span className="elapsed">{elapsed(from || Date.now())}</span>
    </div>
  );
}

/** 对话流主体（总管家 tab）。 */
export function ChatView({ engine, onOpenWorker }: { engine: ButlerEngine; onOpenWorker?: (id: string) => void }) {
  currentEngine = engine;
  const { snapshot, delegate, resolveApproval, generateMorning } = useButler(engine);
  const [input, setInput] = useState("");
  const scrollRef = useRef<HTMLDivElement>(null);
  const taRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [snapshot.conversation, snapshot.escalations, snapshot.busy]);

  // textarea 高度自适应
  useEffect(() => {
    const ta = taRef.current;
    if (!ta) return;
    ta.style.height = "auto";
    ta.style.height = Math.min(92, ta.scrollHeight) + "px";
  }, [input]);

  const send = () => {
    const text = input.trim();
    if (!text || snapshot.busy) return;
    setInput("");
    void delegate(text);
  };

  return (
    <div className="bt-mainpane">
      <Styles />
      <div className="bt-scroll" ref={scrollRef}>
        {snapshot.settings.killSwitch && (
          <div className="bt-banner">自治已暂停（kill switch 开启）。可继续手动委派，但不会自动盯进度或代批。</div>
        )}
        <div className="bt-flow">
          {snapshot.conversation.map((n) => (
            <Notice
              key={n.id}
              n={n}
              // 仅当该会话仍在 tabstrip 里时才可跳转（已清理的历史卡点了会空白）
              onOpenWorker={
                onOpenWorker && n.workerId && snapshot.workers.some((w) => w.sessionId === n.workerId)
                  ? onOpenWorker
                  : undefined
              }
            />
          ))}

          {snapshot.escalations.map((e) => (
            <EscalationCard
              key={e.item.requestId}
              e={e}
              onResolve={(d) => void resolveApproval(e.workerId, e.item.requestId, d)}
            />
          ))}

          {snapshot.busy && <Activity phase={snapshot.phase} from={snapshot.turnStartedMs} />}
        </div>
      </div>

      <div className="bt-composer">
        <div className="bt-capsule">
          <textarea
            ref={taRef}
            rows={1}
            value={input}
            placeholder="交给总管家：一句话描述目标，它会拆解并派发；也可直接聊"
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                send();
              }
            }}
          />
          <button className="bt-send" onClick={send} disabled={!input.trim() || snapshot.busy} aria-label="发送">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M12 19V5" />
              <path d="m5 12 7-7 7 7" />
            </svg>
          </button>
        </div>
        <div className="bt-hint">
          <span>Enter 发送 · Shift+Enter 换行</span>
          <span className="model" onClick={generateMorning}>生成晨报</span>
        </div>
      </div>
    </div>
  );
}

export { Activity, Prose, ResultCard };
export type { ButlerSnapshot, ButlerWorker, Discipline };
