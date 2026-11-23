// 晨间简报（Morningside）：打开工作台的第一屏。
// 纯读接口聚合：listSessions + getTodos + getPending + getGoal，不写会话、不做副作用；
// 组件一律经 props 接收 agent（不在闭包捕获 ctx），对齐 chatapp/task-manager 单元形态。
// 构建：esbuild src/main.tsx → dist/main.js（react 系 external，宿主 import map 供给）。
import type {
  AgentChannel,
  AppContext,
  GoalInfo,
  PendingItem,
  SessionListItem,
} from "@ava-ai/app-sdk";
import { useCallback, useEffect, useState } from "react";

const CSS = `
.ms-root{height:100%;overflow:auto;box-sizing:border-box;font-size:13px;line-height:1.6;
  color:var(--ava-label-primary)}
.ms-inner{padding:var(--ava-space-5) var(--ava-space-5);max-width:760px;margin:0 auto}
.ms-head{margin-bottom:var(--ava-space-4)}
.ms-eyebrow{font-size:11px;color:var(--ava-label-tertiary);letter-spacing:.03em}
.ms-title{margin:var(--ava-space-1) 0 0;font-size:22px;font-weight:700;letter-spacing:-.4px;line-height:1.25}
.ms-sub{margin:var(--ava-space-1) 0 0;font-size:13px;color:var(--ava-label-secondary)}
.ms-card{border:0.5px solid var(--ava-border-l2);border-radius:var(--ava-radius-md);
  background:var(--ava-bg-layer-1);padding:var(--ava-space-4);margin-top:var(--ava-space-3)}
.ms-card-hd{display:flex;align-items:center;gap:var(--ava-space-2);margin-bottom:var(--ava-space-3)}
.ms-card-title{font-size:12px;font-weight:600;margin:0}
.ms-card-hd .ms-spacer{flex:1}
.ms-chip{display:inline-flex;align-items:center;height:20px;padding:0 8px;border-radius:var(--ava-radius-pill);
  font-size:10.5px;color:var(--ava-label-tertiary);background:var(--ava-interactive-bg-hover)}
.ms-item{display:flex;align-items:flex-start;gap:var(--ava-space-3);padding:var(--ava-space-2) 0}
.ms-item+.ms-item{border-top:0.5px solid var(--ava-border-l1)}
.ms-idx{width:22px;height:22px;border-radius:50%;flex:0 0 auto;display:grid;place-items:center;
  font-size:11px;font-weight:700;background:var(--ava-interactive-bg-hover);color:var(--ava-label-secondary)}
.ms-item.hot .ms-idx{background:var(--ava-state-business-primary);color:#fff}
.ms-body{flex:1;min-width:0}
.ms-line{font-size:13px;font-weight:600}
.ms-item .ms-line{font-weight:500}
.ms-meta{font-size:11px;color:var(--ava-label-tertiary);margin-top:2px}
.ms-meta .mono{font-family:var(--ava-font-mono)}
.ms-empty{color:var(--ava-label-tertiary);font-size:12px;text-align:center;padding:var(--ava-space-5) 0}
.ms-err{border:0.5px solid var(--ava-border-l2);border-radius:var(--ava-radius-md);
  padding:var(--ava-space-4);color:var(--ava-label-secondary);font-size:12px;line-height:1.8}
.ms-actions{display:flex;gap:var(--ava-space-2);margin-top:var(--ava-space-2);flex-wrap:wrap}
.ms-btn{height:26px;padding:0 12px;border-radius:var(--ava-radius-pill);border:0.5px solid var(--ava-border-l2);
  background:transparent;color:var(--ava-label-secondary);font:inherit;font-size:11px;cursor:pointer}
.ms-btn:hover{background:var(--ava-interactive-bg-hover)}
.ms-btn.go{color:var(--ava-state-success);border-color:color-mix(in srgb,var(--ava-state-success) 40%,transparent)}
.ms-btn.danger{color:var(--ava-state-danger);border-color:color-mix(in srgb,var(--ava-state-danger) 40%,transparent)}
.ms-btn.primary{background:var(--ava-state-business-primary);border-color:var(--ava-state-business-primary);color:#fff}
.ms-question{font-size:13px;font-weight:600}
.ms-opt{display:inline-flex;align-items:center;height:26px;padding:0 12px;border-radius:var(--ava-radius-pill);
  border:0.5px solid var(--ava-border-l2);background:var(--ava-bg-layer-1);font:inherit;font-size:12px;
  cursor:pointer;color:var(--ava-label-primary)}
.ms-opt:hover{border-color:var(--ava-state-business-primary);color:var(--ava-state-business-primary)}
.ms-opt.on{background:var(--ava-state-business-primary);border-color:var(--ava-state-business-primary);color:#fff}
.ms-phase{font-size:10.5px;font-weight:600;letter-spacing:.02em}
.ms-phase.active{color:var(--ava-state-business-primary)}
.ms-phase.paused{color:var(--ava-state-warning)}
.ms-phase.blocked{color:var(--ava-state-danger)}
.ms-phase.complete{color:var(--ava-state-success)}
.ms-live{width:7px;height:7px;border-radius:50%;background:var(--ava-state-success);flex:0 0 auto}
.ms-goal{padding:var(--ava-space-4);display:flex;flex-direction:column;gap:var(--ava-space-3);height:100%;overflow:auto}
.ms-ring{display:block;margin:0 auto}
.ms-goal-obj{font-size:13px;font-weight:600;text-align:center;line-height:1.5}
.ms-kv{display:flex;justify-content:space-between;font-size:12px;color:var(--ava-label-secondary);padding:2px 0}
.ms-kv b{color:var(--ava-label-primary);font-weight:600;font-family:var(--ava-font-mono)}
.ms-tl{height:100%;overflow:auto;padding:var(--ava-space-4);display:flex;flex-direction:column;gap:var(--ava-space-2)}
.ms-tl-item{border:0.5px solid var(--ava-border-l2);border-radius:var(--ava-radius-md);
  background:var(--ava-bg-layer-1);padding:var(--ava-space-2) var(--ava-space-3)}
.ms-tl-item .hd{display:flex;align-items:center;gap:var(--ava-space-2)}
.ms-tl-item .title{font-size:12px;font-weight:600;flex:1;min-width:0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.ms-tl-item .meta{font-size:10.5px;color:var(--ava-label-tertiary);margin-top:2px;font-family:var(--ava-font-mono)}
`;

const PHASE_LABEL: Record<string, string> = {
  active: "进行中",
  paused: "已暂停",
  blocked: "受阻",
  complete: "已完成",
  "": "无目标",
};

/** 相对时间（epoch ms → 中文短文本）。 */
function relTime(ms?: number): string {
  if (!ms) return "—";
  const diff = Date.now() - ms;
  const m = Math.floor(diff / 60000);
  if (m < 1) return "刚刚";
  if (m < 60) return `${m} 分钟前`;
  const hh = Math.floor(m / 60);
  if (hh < 24) return `${hh} 小时前`;
  const d = Math.floor(hh / 24);
  if (d < 7) return `${d} 天前`;
  return new Date(ms).toLocaleDateString("zh-CN");
}

interface PendingRow {
  sessionId: string;
  sessionTitle: string;
  item: PendingItem;
}

interface Brief {
  sessions: SessionListItem[];
  todos: { sessionId: string; sessionTitle: string; content: string; status: string }[];
  pending: PendingRow[];
  goal: { sessionId: string; sessionTitle: string; goal: GoalInfo } | null;
}

/** 聚合简报数据（纯读，容错：单会话失败不拖垮整体）。 */
async function loadBrief(agent: AgentChannel): Promise<Brief> {
  const sessions = (await agent.listSessions()) || [];
  const recent = [...sessions].sort((a, b) => (b.updatedAtMs || 0) - (a.updatedAtMs || 0)).slice(0, 6);

  const todos: Brief["todos"] = [];
  const pending: PendingRow[] = [];
  let goal: Brief["goal"] = null;

  for (const s of recent) {
    try {
      const t = (await agent.getTodos(s.id)) || [];
      for (const x of t) {
        if (x.status !== "completed") {
          todos.push({ sessionId: s.id, sessionTitle: s.title, content: x.content, status: x.status });
        }
      }
    } catch (_) { /* 单会话失败跳过 */ }
    try {
      const p = (await agent.getPending(s.id)) || [];
      for (const it of p) pending.push({ sessionId: s.id, sessionTitle: s.title, item: it });
    } catch (_) { /* 单会话失败跳过 */ }
    if (!goal) {
      try {
        const g = await agent.getGoal(s.id);
        if (g && g.phase !== "complete") goal = { sessionId: s.id, sessionTitle: s.title, goal: g };
      } catch (_) { /* 单会话失败跳过 */ }
    }
  }
  return { sessions: recent, todos, pending, goal };
}

function useBrief(agent: AgentChannel) {
  const [state, setState] = useState<{ loading: boolean; error: Error | null; data: Brief | null }>({
    loading: true,
    error: null,
    data: null,
  });

  const reload = useCallback(() => {
    let alive = true;
    setState((s) => ({ ...s, loading: true }));
    loadBrief(agent)
      .then((data) => alive && setState({ loading: false, error: null, data }))
      .catch((e: unknown) => {
        alive && setState({ loading: false, error: e instanceof Error ? e : new Error(String(e)), data: null });
      });
    return () => {
      alive = false;
    };
  }, [agent]);

  useEffect(reload, [reload]);

  return { ...state, reload };
}

function Styles() {
  useEffect(() => {
    if (document.getElementById("ms-styles")) return;
    const el = document.createElement("style");
    el.id = "ms-styles";
    el.textContent = CSS;
    document.head.appendChild(el);
  }, []);
  return null;
}

function Main({ agent }: { agent: AgentChannel }) {
  const { loading, error, data, reload } = useBrief(agent);
  if (loading) return null;
  if (error) {
    return (
      <div className="ms-root">
        <Styles />
        <div className="ms-inner">
          <div className="ms-err">
            边车不可达，无法汇总简报：{error.message}。确认 ava2 实例已启动后刷新重试。
          </div>
        </div>
      </div>
    );
  }
  const { sessions, todos, pending, goal } = data!;
  const inProgress = todos.filter((t) => t.status === "in_progress");
  const rest = todos.filter((t) => t.status === "pending");
  const top3 = [...inProgress, ...rest].slice(0, 3);

  const approvals = pending.filter((p) => p.item.kind === "approval");
  const planReviews = pending.filter((p) => p.item.kind === "plan_review");
  const questions = pending.filter((p) => p.item.kind === "question");

  return (
    <div className="ms-root">
      <Styles />
      <div className="ms-inner">
        <header className="ms-head">
          <div className="ms-eyebrow">
            {new Date().toLocaleDateString("zh-CN", { weekday: "long", month: "long", day: "numeric" })}
          </div>
          <h1 className="ms-title">早上好。这里是今天的概况。</h1>
          <p className="ms-sub">由你的会话、待办与目标实时汇总；点任何一条回填项即可处理。</p>
        </header>

        <section className="ms-card">
          <div className="ms-card-hd">
            <h2 className="ms-card-title">今日三件要事</h2>
            <div className="ms-spacer" />
            <span className="ms-chip">待办 {todos.length}</span>
          </div>
          {top3.length === 0 ? (
            <div className="ms-empty">没有未完成的待办——放松一下，或去会话里派个新任务。</div>
          ) : (
            top3.map((t, i) => (
              <div className={"ms-item" + (i === 0 ? " hot" : "")} key={t.sessionId + ":" + t.content}>
                <div className="ms-idx">{String(i + 1)}</div>
                <div className="ms-body">
                  <div className="ms-line">{t.content}</div>
                  <div className="ms-meta">
                    <span className="mono">{t.sessionId.slice(0, 8)}</span> · {t.sessionTitle}
                  </div>
                </div>
              </div>
            ))
          )}
        </section>

        <section className="ms-card">
          <div className="ms-card-hd">
            <h2 className="ms-card-title">待回填</h2>
            <div className="ms-spacer" />
            <span className="ms-chip">{String(approvals.length + planReviews.length)}</span>
          </div>
          {approvals.length + planReviews.length === 0 ? (
            <div className="ms-empty">暂无需要你回填的审批或计划评审。</div>
          ) : (
            [...approvals, ...planReviews].map((p) => (
              <div className="ms-item" key={p.item.requestId}>
                <div className="ms-body">
                  <div className="ms-line">
                    {p.item.kind === "approval" ? `审批：${p.item.tool || "工具执行"}` : "计划评审"}
                  </div>
                  <div className="ms-meta">
                    <span className="mono">{p.sessionId.slice(0, 8)}</span> · {p.sessionTitle}
                  </div>
                  <div className="ms-actions">
                    {p.item.kind === "approval" ? (
                      <>
                        <button className="ms-btn go" onClick={() => agent.replyApproval(p.sessionId, p.item.requestId, "approve").then(reload)}>批准</button>
                        <button className="ms-btn danger" onClick={() => agent.replyApproval(p.sessionId, p.item.requestId, "deny").then(reload)}>拒绝</button>
                      </>
                    ) : (
                      <>
                        <button className="ms-btn go" onClick={() => agent.replyPlanReview(p.sessionId, p.item.requestId, "approve").then(reload)}>通过</button>
                        <button className="ms-btn danger" onClick={() => agent.replyPlanReview(p.sessionId, p.item.requestId, "keep_planning").then(reload)}>继续规划</button>
                      </>
                    )}
                  </div>
                </div>
              </div>
            ))
          )}
        </section>

        {questions.length > 0 && (
          <section className="ms-card">
            <div className="ms-card-hd">
              <h2 className="ms-card-title">需要你决策</h2>
              <div className="ms-spacer" />
              <span className="ms-chip">{String(questions.length)}</span>
            </div>
            {questions.map((p) => (
              <QuestionBlock key={p.item.requestId} row={p} agent={agent} reload={reload} />
            ))}
          </section>
        )}

        <section className="ms-card">
          <div className="ms-card-hd">
            <h2 className="ms-card-title">最近会话</h2>
            <div className="ms-spacer" />
            <span className="ms-chip">{String(sessions.length)}</span>
          </div>
          {sessions.length === 0 ? (
            <div className="ms-empty">还没有会话。去对话页开一个吧。</div>
          ) : (
            sessions.map((s) => (
              <div className="ms-item" key={s.id}>
                {s.live ? <span className="ms-live" style={{ marginTop: "6px" }} /> : null}
                <div className="ms-body">
                  <div className="ms-line">{s.title || "（无标题）"}</div>
                  <div className="ms-meta">
                    {relTime(s.updatedAtMs)} · <span className="mono">{s.id.slice(0, 8)}</span>
                  </div>
                </div>
                {s.goalPhase ? (
                  <span className={"ms-phase " + s.goalPhase}>{PHASE_LABEL[s.goalPhase] || s.goalPhase}</span>
                ) : null}
              </div>
            ))
          )}
        </section>
      </div>
    </div>
  );
}

function QuestionBlock({
  row,
  agent,
  reload,
}: {
  row: PendingRow;
  agent: AgentChannel;
  reload: () => void;
}) {
  const q = (row.item.questions && row.item.questions[0]) || null;
  const [sel, setSel] = useState<string[]>([]);
  if (!q) {
    return (
      <div className="ms-item">
        <div className="ms-body">
          <div className="ms-question">会话向你提出了一个问询（{row.sessionTitle}）</div>
          <div className="ms-actions">
            <button className="ms-btn primary" onClick={() => agent.answerQuestion(row.sessionId, row.item.requestId, []).then(reload)}>跳过</button>
          </div>
        </div>
      </div>
    );
  }
  const toggle = (label: string) => {
    setSel((prev) =>
      q.multiSelect ? (prev.includes(label) ? prev.filter((x) => x !== label) : [...prev, label]) : [label],
    );
  };
  return (
    <div className="ms-item">
      <div className="ms-body">
        <div className="ms-question">{q.header || q.question}</div>
        <div className="ms-meta">来自 {row.sessionTitle}</div>
        <div className="ms-actions">
          {q.options.map((o) => (
            <button
              key={o.label}
              className={"ms-opt" + (sel.includes(o.label) ? " on" : "")}
              onClick={() => toggle(o.label)}
              title={o.description || ""}
            >
              {o.label}
            </button>
          ))}
          <button
            className="ms-btn primary"
            onClick={() => agent.answerQuestion(row.sessionId, row.item.requestId, [{ id: q.id, selected: sel }]).then(reload)}
          >
            提交
          </button>
        </div>
      </div>
    </div>
  );
}

function GoalRing({ goal }: { goal: GoalInfo }) {
  const max = goal.maxGoalRounds || 1;
  const pct = goal.phase === "complete" ? 1 : Math.min(1, goal.roundsStarted / max);
  const C = 2 * Math.PI * 40;
  const color = goal.phase === "blocked" ? "var(--ava-state-danger)" : "var(--ava-state-business-primary)";
  return (
    <svg className="ms-ring" width={96} height={96} viewBox="0 0 100 100">
      <circle cx={50} cy={50} r={40} fill="none" stroke="var(--ava-interactive-bg-hover)" strokeWidth={9} />
      <circle
        cx={50} cy={50} r={40} fill="none" stroke={color} strokeWidth={9} strokeLinecap="round"
        strokeDasharray={C} strokeDashoffset={C * (1 - pct)} transform="rotate(-90 50 50)"
      />
      <text x={50} y={47} textAnchor="middle" style={{ fontSize: 18, fontWeight: 700, fill: "var(--ava-label-primary)" }}>
        {Math.round(pct * 100)}%
      </text>
      <text x={50} y={63} textAnchor="middle" style={{ fontSize: 8, fill: "var(--ava-label-tertiary)" }}>目标进度</text>
    </svg>
  );
}

function GoalPanel({ agent }: { agent: AgentChannel }) {
  const { loading, error, data } = useBrief(agent);
  if (loading) return null;
  return (
    <div className="ms-root">
      <Styles />
      {error || !data || !data.goal ? (
        <div className="ms-goal">
          <div className="ms-empty">暂无进行中的目标。</div>
          <div className="ms-empty" style={{ paddingTop: 0 }}>在会话中设定目标后，这里会显示进度环。</div>
        </div>
      ) : (
        <div className="ms-goal">
          <GoalRing goal={data.goal.goal} />
          <div className="ms-goal-obj">{data.goal.goal.objective}</div>
          <div>
            <div className="ms-kv"><span>阶段</span><b>{PHASE_LABEL[data.goal.goal.phase] || data.goal.goal.phase}</b></div>
            <div className="ms-kv"><span>轮次</span><b>{data.goal.goal.roundsStarted}/{data.goal.goal.maxGoalRounds || "∞"}</b></div>
            <div className="ms-kv"><span>会话</span><b>{data.goal.sessionId.slice(0, 8)}</b></div>
          </div>
        </div>
      )}
    </div>
  );
}

function Timeline({ agent }: { agent: AgentChannel }) {
  const { loading, error, data } = useBrief(agent);
  if (loading) return null;
  const sessions = data && data.sessions ? data.sessions : [];
  return (
    <div className="ms-root">
      <Styles />
      <div className="ms-tl">
        {error ? (
          <div className="ms-err">边车不可达：{error.message}</div>
        ) : sessions.length === 0 ? (
          <div className="ms-empty">暂无会话</div>
        ) : (
          sessions.map((s) => (
            <div className="ms-tl-item" key={s.id}>
              <div className="hd">
                {s.live ? <span className="ms-live" /> : null}
                <div className="title">{s.title || "（无标题）"}</div>
                {s.goalPhase ? <span className={"ms-phase " + s.goalPhase}>{PHASE_LABEL[s.goalPhase] || s.goalPhase}</span> : null}
              </div>
              <div className="meta">
                {relTime(s.updatedAtMs)} · {s.id.slice(0, 8)}
                {s.workingDir ? " · " + s.workingDir : ""}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

export function setup(ctx: AppContext): { dispose(): void } {
  const disposers = [
    ctx.slots.inject("nav.sidebar", {
      order: 5,
      viewId: "morningside",
      title: "晨间简报",
      icon: "M12 2a1 1 0 0 1 1 1v1.05a8 8 0 1 1-2 0V3a1 1 0 0 1 1-1zM5 16a1 1 0 0 1 1 1v2h12v-2a1 1 0 0 1 2 0v3a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1v-3a1 1 0 0 1 1-1z",
    }),
    ctx.slots.inject("surface.main", {
      viewId: "morningside",
      title: "晨间简报",
      component: async () => ({ default: () => <Main agent={ctx.agent} /> }),
    }),
    ctx.slots.inject("surface.canvas", {
      viewId: "morningside-canvas",
      title: "会话时间轴",
      component: async () => ({ default: () => <Timeline agent={ctx.agent} /> }),
    }),
    ctx.slots.inject("panel.sidebar", {
      order: 5,
      viewId: "morningside",
      title: "目标进度",
      component: async () => ({ default: () => <GoalPanel agent={ctx.agent} /> }),
    }),
  ];
  return {
    dispose() {
      for (const d of disposers) d.dispose();
    },
  };
}
