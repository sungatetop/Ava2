// 管家代管引擎（App 层，不动 onion 边车）。
//
// 「App 即 agent」：App 是常驻的自主实体——观察（events/轮询）→ 决策 → 行动
// （AgentChannel）→ 记忆（storage）循环。代管的「脑」分两档：
//   - 安全纪律（硬约束）：规则引擎白名单，高危一律升级，不交给 LLM 自由裁量；
//   - 委派拆解（软决策）：借力思考会话（复用同一 thinker 会话，发决策 prompt，
//     读回 assistant 文本 → JSON 子任务数组），失败/超时回退单任务。
//
// 能力面（全部来自 @ava-ai/app-sdk 的 AgentChannel）：
//   - createSession / send         替用户建会话派任务
//   - onSessionEvent               实时收 approval_requested / turn_report / session_closed
//   - getPending / replyApproval   代批审批
//   - listSessions                 刷新 worker 台账状态
import type { AgentChannel, AppContext, Disposable, PendingItem, SessionListItem } from "@ava-ai/app-sdk";
import { useCallback, useEffect, useState } from "react";

export type StorageLike = AppContext["storage"];

export type Discipline = "escalate-only" | "disciplined" | "relaxed";

export interface ButlerSettings {
  killSwitch: boolean;
  quietHours: { start: string; end: string };
  discipline: Discipline;
}

export const DEFAULT_SETTINGS: ButlerSettings = {
  killSwitch: false,
  quietHours: { start: "22:00", end: "08:00" },
  discipline: "disciplined",
};

export type WorkerStatus = "running" | "done";

export interface ButlerWorker {
  sessionId: string;
  task: string;
  status: WorkerStatus;
  createdAtMs: number;
  updatedAtMs: number;
  /** 当前阶段文案（运行态活动条展示）。 */
  phase?: string;
  /** 收尾时间（工作台「今日结算」与相对时间用）。 */
  doneAtMs?: number;
  lastEvent?: string;
}

/** 对话流条目（管家发言与用户委派同源持久化）。 */
export interface ButlerNotice {
  id: string;
  role: "user" | "butler";
  kind: "text" | "delegate" | "report" | "morning" | "failure";
  text: string;
  workerId?: string;
  ts: number;
  /** morning 专属：简报要点。 */
  summary?: string[];
  /** failure 专属：可供重试的原始委派文本。 */
  retryText?: string;
  /** report 专属：执行结果正文（与 text 分离，便于折叠）。 */
  body?: string;
}

/** 升级审批（纪律裁决为「不代批、交用户」的项）。 */
export interface Escalation {
  workerId: string;
  sessionTitle: string;
  item: PendingItem;
}

export interface ButlerSnapshot {
  ready: boolean;
  /** 管家正在思考/派发（UI 显示运行态活动条）。 */
  busy: boolean;
  /** 忙碌阶段文案。 */
  phase: string;
  /** 本轮开始时间戳（活动条计时）。 */
  turnStartedMs: number;
  settings: ButlerSettings;
  conversation: ButlerNotice[];
  workers: ButlerWorker[];
  escalations: Escalation[];
}

/** 管家对一句话的决策：先回应，再决定是否派发子任务。 */
interface BrainPlan {
  reply: string;
  subtasks: string[];
}

/** 工具风险分级：高危 = 硬护栏升级；低危 = 可代批；未知 = 按档位裁决。 */
const HIGH_RISK_HINTS = [
  "publish", "push", "deploy", "delete", "drop", "remove", "rm ", "uninstall",
  "pay", "payment", "billing", "credential", "secret", "token", "api_key", "apikey",
  "force", "sudo", "reset", "migrate", "truncate", "purge",
];
const LOW_RISK_HINTS = [
  "build", "test", "lint", "fmt", "format", "read", "cat", "ls", "grep", "find",
  "backup", "install", "echo", "list", "get", "status", "diff", "log", "check",
  "version", "watch", "typecheck", "tsc",
];

export function assessToolRisk(tool: string | undefined): "high" | "low" | "unknown" {
  const t = (tool || "").toLowerCase();
  if (HIGH_RISK_HINTS.some((h) => t.includes(h))) return "high";
  if (LOW_RISK_HINTS.some((h) => t.includes(h))) return "low";
  return "unknown";
}

export type ApprovalDecision = "approve" | "escalate";

/**
 * 纪律裁决：仅升级 / 纪律代批 / 宽松代批 + 高危硬护栏。
 * killSwitch 置位时一律升级（暂停自治）。
 */
export function decideApproval(
  item: PendingItem,
  settings: ButlerSettings,
): ApprovalDecision {
  if (settings.killSwitch) return "escalate";
  if (settings.discipline === "escalate-only") return "escalate";
  const risk = assessToolRisk(item.tool);
  if (risk === "high") return "escalate";
  if (risk === "low") return "approve";
  // unknown：纪律代批下「不确定即升级」，宽松代批下放宽
  return settings.discipline === "relaxed" ? "approve" : "escalate";
}

function idOf(prefix: string): string {
  return prefix + Date.now() + Math.random().toString(36).slice(2, 6);
}

function now(): number {
  return Date.now();
}

function isAssistant(e: unknown): boolean {
  return (e as Record<string, unknown> | undefined)?.type === "assistant_message";
}

/** 从 assistant_message 帧提取纯文本（message.blocks[] 中 type==="text" 的 text 拼接）。 */
function textBlocks(message: unknown): string {
  const m = message as Record<string, unknown> | undefined;
  const blocks = Array.isArray(m?.blocks) ? (m!.blocks as unknown[]) : [];
  return blocks
    .map((b) => {
      const r = b as Record<string, unknown> | undefined;
      return r?.type === "text" ? String(r.text ?? "") : "";
    })
    .join("");
}

/** 从 LLM 回复中解析决策 JSON：{ reply, subtasks }。
 *  兼容三种形态：完整对象 / 裸数组（视为 subtasks）/ 纯文本（视为 reply）。
 *  回复可能含多帧拼接/前后缀噪声，取**首个** `{...}` 或 `[...]` 块尝试解析。 */
function parsePlan(raw: string): BrainPlan {
  const text = (raw || "").trim();
  if (!text) return { reply: "", subtasks: [] };
  const objMatch = text.match(/\{[\s\S]*?\}/);
  if (objMatch) {
    try {
      const o = JSON.parse(objMatch[0]) as Record<string, unknown>;
      const reply = typeof o.reply === "string" ? o.reply.trim() : "";
      const arr = Array.isArray(o.subtasks) ? o.subtasks : [];
      const subtasks = arr
        .filter((x): x is string => typeof x === "string" && x.trim().length > 0)
        .map((x) => x.trim());
      if (reply || subtasks.length) return { reply, subtasks };
    } catch {
      /* 落到下面的兜底 */
    }
  }
  const arrMatch = text.match(/\[.*?\]/);
  if (arrMatch) {
    try {
      const arr = JSON.parse(arrMatch[0]);
      if (Array.isArray(arr)) {
        const subtasks = arr
          .filter((x): x is string => typeof x === "string" && x.trim().length > 0)
          .map((x) => x.trim());
        if (subtasks.length) return { reply: "", subtasks };
      }
    } catch {
      /* 落到纯文本兜底 */
    }
  }
  // 纯文本回复：整段作为管家回应（去掉可能的代码围栏）
  return { reply: text.replace(/^```[a-z]*\n?|```$/g, "").trim(), subtasks: [] };
}

const POLL_MS = 8000;
/** worker 超过该时长仍 running 且无待批时，读回复判定是否已完成（事件丢帧兜底）。 */
const STALE_MS = 45_000;
/** 持久化结构版本：与旧格式不兼容时清空对话与台账（见 start）。 */
const SCHEMA = 2;

/** 纯逻辑代管引擎：不依赖 React，可单测。 */
export class ButlerEngine {
  private settings: ButlerSettings = DEFAULT_SETTINGS;
  private workers: ButlerWorker[] = [];
  private conversation: ButlerNotice[] = [];
  private disposers: Array<{ dispose(): void }> = [];
  private timer: ReturnType<typeof setInterval> | null = null;
  private listeners = new Set<(s: ButlerSnapshot) => void>();
  private ready = false;
  /** 思考会话 id（借力 LLM 决策复用，不重复建会话）。 */
  private thinkerId: string | null = null;
  /** 决策锁：同一时刻只允许一个 LLM 决策进行（串行化避免回复串扰）。 */
  private thinking = false;
  /** 最近一次决策失败的真实原因（供失败卡展示，避免一律显示「未响应」）。 */
  private lastBrainError = "";
  /** 管家忙碌态（思考/派发中），驱动 UI「正在思考…」。 */
  private busy = false;
  /** 忙碌阶段文案（运行态活动条展示）。 */
  private phase = "";
  /** 当前轮开始时间（活动条计时）。 */
  private turnStartedMs = 0;

  constructor(
    private agent: AgentChannel,
    private storage: StorageLike,
  ) {}

  private get snapshot(): ButlerSnapshot {
    return {
      ready: this.ready,
      busy: this.busy,
      phase: this.phase,
      turnStartedMs: this.turnStartedMs,
      settings: this.settings,
      conversation: this.conversation,
      workers: this.workers,
      escalations: this._escalations,
    };
  }

  private _escalations: Escalation[] = [];

  private notify() {
    const s = this.snapshot;
    for (const fn of this.listeners) fn(s);
  }

  subscribe(fn: (s: ButlerSnapshot) => void): () => void {
    this.listeners.add(fn);
    fn(this.snapshot);
    return () => {
      this.listeners.delete(fn);
    };
  }

  private async persistWorkers() {
    await this.storage.set("workers", this.workers).catch(() => {});
  }
  private async persistConversation() {
    await this.storage.set("conversation", this.conversation).catch(() => {});
  }
  private async persistSettings() {
    await this.storage.set("settings", this.settings).catch(() => {});
  }
  private async persistEscalations() {
    await this.storage.set("escalations", this._escalations).catch(() => {});
  }

  private pushNotice(n: Omit<ButlerNotice, "id" | "ts">) {
    this.conversation.push({ ...n, id: idOf("n"), ts: now() });
    this.persistConversation();
  }

  /** 启动代管：加载持久化状态 → 恢复订阅 running workers → 启动轮询。 */
  async start() {
    const [workers, conv, settings, escalations, schema] = await Promise.all([
      this.storage.get<ButlerWorker[]>("workers"),
      this.storage.get<ButlerNotice[]>("conversation"),
      this.storage.get<ButlerSettings>("settings"),
      this.storage.get<Escalation[]>("escalations"),
      this.storage.get<number>("schema"),
    ]);
    // 存储结构迁移：旧版本条目（会话号文案、无正文的结果卡）会污染新界面，
    // 版本不符时清空对话/台账并丢弃旧思考会话（其历史里堆着旧格式计划）。
    const fresh = schema !== SCHEMA;
    if (fresh) {
      await this.storage.set("schema", SCHEMA).catch(() => {});
      await this.storage.set("thinkerId", null).catch(() => {});
      this.thinkerId = null;
    }
    this.workers = !fresh && Array.isArray(workers) ? workers : [];
    this._escalations = !fresh && Array.isArray(escalations) ? escalations : [];
    if (!fresh && Array.isArray(conv) && conv.length) {
      this.conversation = conv;
    } else {
      this.conversation = [{
        id: idOf("n"), role: "butler", kind: "text", ts: now(),
        text: "你好，我是总管家。给我一句话，我会拆解成任务派给独立会话执行，再把结果收回来汇报给你。",
      }];
      this.persistConversation();
    }
    if (fresh) {
      this.persistWorkers();
      this.persistEscalations();
    }
    this.settings = settings ? { ...DEFAULT_SETTINGS, ...settings, quietHours: { ...DEFAULT_SETTINGS.quietHours, ...(settings.quietHours || {}) } } : DEFAULT_SETTINGS;

    // 恢复订阅仍 running 的 worker（宿主存活期常驻盯进度）
    for (const w of this.workers) {
      if (w.status === "running") this.subscribeWorker(w.sessionId);
    }

    this.ready = true;
    this.timer = setInterval(() => void this.poll(), POLL_MS);
    void this.poll();
    this.notify();
  }

  stop() {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
    for (const d of this.disposers) d.dispose();
    this.disposers = [];
    this.ready = false;
    this.notify();
  }

  private subscribeWorker(sessionId: string) {
    const d = this.agent.onSessionEvent(sessionId, (frame) => this.onEvent(sessionId, frame));
    this.disposers.push(d);
  }

  /** 事件帧处理：turn 收束 / session_closed → 本轮完成；approval_requested → 触发一次代批扫描。
   *  注意：`live` 只表示会话进程存在，任务跑完后会话仍 live，故不能靠 live 判断完成，
   *  真正的完成信号是 turn_end（一轮完整 assistant 回复结束）与 session_closed。 */
  private onEvent(sessionId: string, frame: { type: string } & Record<string, unknown>) {
    const w = this.workers.find((x) => x.sessionId === sessionId);
    if (!w) return;
    const type = frame.type as string;
    if (type === "turn_end" || type === "session_closed") {
      void this.finishWorker(w, type);
    } else if (type === "turn_start") {
      w.updatedAtMs = now();
      w.phase = "执行中";
      this.persistWorkers();
    } else if (type === "approval_requested") {
      void this.scanApprovals();
    }
    this.notify();
  }

  /** 轮询：刷新 worker 存活态 + 扫描待批（事件丢帧兜底）。 */
  private async poll() {
    if (!this.ready || this.settings.killSwitch) return;
    await this.refreshWorkers();
    await this.scanApprovals();
    this.notify();
  }

  /** 轮询兜底：事件丢帧时，用会话自身的 `busy` 判定本轮是否已收束
   *  （`live` 只表示进程存在，不能用来判完成），收尾时回读最后回复。 */
  private async refreshWorkers() {
    let sessions: SessionListItem[] = [];
    try {
      sessions = (await this.agent.listSessions()) || [];
    } catch (_) {
      return;
    }
    const byId = new Map(sessions.map((s) => [s.id, s]));
    for (const w of this.workers) {
      if (w.status !== "running") continue;
      // 会话已不存在（被清理）→ 直接收尾
      if (!byId.has(w.sessionId)) {
        await this.finishWorker(w, "session_missing");
        continue;
      }
      // 刚派发不久：留足起步时间，避免把「还没开跑」误判为已完成
      if (now() - w.createdAtMs < STALE_MS) continue;
      let busy = true;
      try {
        busy = (await this.agent.getSession(w.sessionId)).busy;
      } catch (_) {
        continue;
      }
      if (busy) {
        // 仍在跑：推进阶段文案（tool 执行久时 UI 不至于停在「已派发」）
        if (w.phase !== "执行中") {
          w.phase = "执行中";
          w.updatedAtMs = now();
          this.persistWorkers();
        }
        continue;
      }
      // 空闲但有待回填 → 等审批，不算完成
      let pending: PendingItem[] = [];
      try {
        pending = (await this.agent.getPending(w.sessionId)) || [];
      } catch (_) {
        continue;
      }
      if (pending.length > 0) continue;
      await this.finishWorker(w, "poll_idle");
    }
  }

  /** 扫描待回填：对每个 running worker 的审批按纪律裁决。 */
  private async scanApprovals() {
    if (!this.ready || this.settings.killSwitch) return;
    for (const w of this.workers) {
      if (w.status !== "running") continue;
      let pending: PendingItem[] = [];
      try {
        pending = (await this.agent.getPending(w.sessionId)) || [];
      } catch (_) {
        continue;
      }
      for (const item of pending) {
        if (item.kind !== "approval" && item.kind !== "plan_review") continue;
        // 已在升级列表里跳过（等用户手动批）
        if (this._escalations.some((e) => e.workerId === w.sessionId && e.item.requestId === item.requestId)) continue;
        const decision = decideApproval(item, this.settings);
        if (decision === "escalate") {
          this._escalations.push({ workerId: w.sessionId, sessionTitle: w.task, item });
          this.persistEscalations();
          this.pushNotice({ role: "butler", kind: "report", workerId: w.sessionId, text: `任务「${w.task}」请求 ${item.tool || item.kind}，需你裁决。` });
        } else {
          try {
            if (item.kind === "plan_review") {
              await this.agent.replyPlanReview(w.sessionId, item.requestId, "approve");
            } else {
              await this.agent.replyApproval(w.sessionId, item.requestId, "approve");
            }
            this.pushNotice({ role: "butler", kind: "text", text: `已按纪律代批：${item.tool || item.kind}（条款 ${item.kind === "plan_review" ? "R1 计划" : "R2 构建/测试"}，任务「${w.task}」）。` });
          } catch (_) {
            // 代批失败（已处理/会话状态变化）——忽略，下一轮对账
          }
        }
      }
    }
  }

  /** 委派：先由管家回应（LLM），再按决策逐个子任务建会话派发 → 登记 worker → 订阅。
   *  `echoUser=false` 用于「重试」：沿用上一条用户气泡，不再重复插入。 */
  async delegate(task: string, opts: { echoUser?: boolean } = {}) {
    const text = task.trim();
    if (!text) return;
    if (opts.echoUser !== false) this.pushNotice({ role: "user", kind: "text", text });
    this.busy = true;
    this.phase = "正在理解你的意图";
    this.turnStartedMs = now();
    this.notify();

    // 软决策：借力思考会话产出 { reply, subtasks }（失败/超时回退：无回应 + 单任务）
    let plan: BrainPlan = { reply: "", subtasks: [] };
    let brainFailed = false;
    try {
      const raw = await this.askBrain(
        `你是用户的「总管家」，负责代用户管理会话与任务。用户刚对你说：\n"""${text}"""\n\n` +
        `请判断这句话的性质并只输出一个 JSON 对象，不要任何解释：\n` +
        `{"reply":"给用户的简短回应（1-3 句，口语化，像管家说话）","subtasks":["需要实际执行的任务，每条一句话"]}\n` +
        `规则：\n` +
        `- 若只是寒暄/提问/闲聊，subtasks 输出 []，reply 正常回答用户。\n` +
        `- 若是需要动手做的事，subtasks 拆成最少数量的可独立执行子任务，reply 说明你打算怎么做。\n` +
        `- reply 必须非空。`,
      );
      plan = parsePlan(raw);
      if (!plan.reply && plan.subtasks.length === 0) {
        brainFailed = true;
        if (!this.lastBrainError) this.lastBrainError = "模型返回的内容无法解析成任务计划";
      }
    } catch {
      brainFailed = true;
      if (!this.lastBrainError) this.lastBrainError = "模型调用异常";
    }

    // 管家回应（这是用户最需要看到的「有回应」）
    if (plan.reply) {
      this.pushNotice({ role: "butler", kind: "text", text: plan.reply });
    } else if (brainFailed) {
      this.pushNotice({
        role: "butler", kind: "failure",
        text: "本轮未完成",
        body: this.lastBrainError || "已保留你的输入，可直接重试；若持续失败请检查模型服务状态。",
        retryText: text,
      });
      this.busy = false;
      this.phase = "";
      this.notify();
      return;
    }

    this.phase = plan.subtasks.length > 0 ? "正在派发会话" : "";
    this.notify();

    for (const sub of plan.subtasks) {
      let created: string | null = null;
      try {
        const s = await this.agent.createSession();
        created = s.id;
        // 先登记并订阅，再发送：本地模型可能在发送后极短时间内收束
        this.workers.push({
          sessionId: s.id, task: sub, status: "running",
          createdAtMs: now(), updatedAtMs: now(),
          phase: "已派发，等待响应",
        });
        this.persistWorkers();
        this.subscribeWorker(s.id);
        this.pushNotice({ role: "butler", kind: "delegate", workerId: s.id, text: sub });
        await this.agent.send(s.id, sub);
      } catch (e) {
        // 发送失败：撤掉刚登记的 worker，避免它永远挂在「运行中」
        if (created) this.workers = this.workers.filter((w) => w.sessionId !== created);
        this.pushNotice({
          role: "butler", kind: "failure",
          text: `派发「${sub}」失败`,
          body: e instanceof Error ? e.message : String(e),
          retryText: sub,
        });
      }
    }
    await this.persistWorkers();
    this.busy = false;
    this.phase = "";
    this.notify();
  }

  /** 重试一次失败的委派：沿用原用户气泡，并移除该失败卡（原地重来，不堆叠历史）。 */
  async retry(text: string, failureId?: string) {
    if (failureId) {
      this.conversation = this.conversation.filter((n) => n.id !== failureId);
      this.persistConversation();
      this.notify();
    }
    await this.delegate(text, { echoUser: false });
  }

  /** 读回某会话最后一条 assistant 文本（durable 历史）。 */
  private async readLastReply(sessionId: string): Promise<string> {
    try {
      const frames = await this.agent.getMessages(sessionId);
      let last = "";
      for (const ev of frames || []) {
        const e = ev as unknown as Record<string, unknown>;
        if (e.type === "assistant_message") {
          const t = textBlocks(e.message);
          if (t.trim()) last = t.trim();
        }
      }
      return last;
    } catch {
      return "";
    }
  }

  /** worker 收尾：标记完成并把真实回复读回对话流（这是「有回应」的关键）。 */
  private async finishWorker(w: ButlerWorker, reason: string) {
    if (w.status === "done") return;
    w.status = "done";
    w.updatedAtMs = now();
    w.doneAtMs = now();
    w.phase = "已完成";
    w.lastEvent = reason;
    this.persistWorkers();
    const reply = await this.readLastReply(w.sessionId);
    this.pushNotice({
      role: "butler", kind: "report", workerId: w.sessionId,
      text: `「${w.task}」已完成`,
      body: reply || "该会话没有产出文本回复，可在会话内查看详情。",
    });
    this.notify();
  }

  /** 确保思考会话存在（复用持久化的 thinkerId，首次则新建）。 */
  private async ensureThinker(): Promise<string> {
    if (this.thinkerId) return this.thinkerId;
    const saved = await this.storage.get<string>("thinkerId").catch(() => undefined);
    if (saved) {
      this.thinkerId = saved;
      return saved;
    }
    const s = await this.agent.createSession();
    this.thinkerId = s.id;
    await this.storage.set("thinkerId", s.id).catch(() => {});
    return s.id;
  }

  /**
   * 借力 LLM 决策：向思考会话发 prompt，等本轮收束（turn_report/turn_end），
   * 再经 getMessages 拉 durable 历史提取**本轮**的 assistant 文本。
   *
   * 注意：思考会话跨轮复用，durable 历史里堆着前几轮的计划 JSON，直接拼接会把
   * 旧轮的计划当成新答复（曾导致「不派发、管家复读旧话」）。故发问前记录
   * assistant 帧数，回读时只取其后新增的帧。
   *
   * 超时（含建会话/发送本身）或失败时返回空串并把真实原因记进 lastBrainError，
   * 由调用方渲染成失败卡。超时阈值给到 120s：本地模型首轮冷启动 + 长篇思考
   * 常见超过 1 分钟，45s 会把仍在正常流式输出的回复打断成「未响应」。
   * 真正超时才打断思考轮，避免其与下一次提问交叉。
   */
  private async askBrain(prompt: string, timeoutMs = 120_000): Promise<string> {
    if (this.thinking) {
      this.lastBrainError = "上一轮决策仍在进行，本次直接回退（不排队）";
      return "";
    }
    this.thinking = true;
    this.lastBrainError = "";
    const round = this.brainRound(prompt);
    let tm: ReturnType<typeof setTimeout> | null = null;
    try {
      const guard = new Promise<never>((_, reject) => {
        tm = setTimeout(() => reject(new Error("timeout")), timeoutMs);
      });
      const raw = await Promise.race([round, guard]);
      if (!raw.trim()) this.lastBrainError = "模型返回了空内容";
      return raw;
    } catch (e) {
      if (e instanceof Error && e.message === "timeout") {
        this.lastBrainError = `决策超时（${Math.round(timeoutMs / 1000)}s 内未响应）`;
        // 打断仍在跑的思考轮，否则它会与下一次委派的提问交叉
        const thinker = this.thinkerId;
        if (thinker) await this.agent.interrupt(thinker).catch(() => undefined);
      } else {
        this.lastBrainError = `模型调用失败：${e instanceof Error ? e.message : String(e)}`;
      }
      return "";
    } finally {
      if (tm) clearTimeout(tm);
      this.thinking = false;
    }
  }

  /** 单轮决策：订阅收束事件 → 发送 → 回读本轮 assistant 文本。
   *  思考会话可能已被销毁（进程重启 / 数据清理）：404 时重建一次再重试，
   *  避免「管家永远报错、只能手动清存储」的死局。 */
  private async brainRound(prompt: string): Promise<string> {
    try {
      return await this.brainRoundOnce(prompt);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (!/session_not_found/i.test(msg)) throw e;
      this.thinkerId = null;
      await this.storage.set("thinkerId", "").catch(() => {});
      return await this.brainRoundOnce(prompt);
    }
  }

  private async brainRoundOnce(prompt: string): Promise<string> {
    const id = await this.ensureThinker();
    const before = (await this.agent.getMessages(id)).filter(isAssistant).length;
    // 先订阅再发送：本地模型可能在同一 tick 内收束，晚订阅会漏帧
    let settle: (() => void) | null = null;
    const ended = new Promise<void>((resolve) => {
      settle = resolve;
    });
    let seen = false;
    const sub = this.agent.onSessionEvent(id, (frame) => {
      const t = (frame as unknown as Record<string, unknown>).type;
      if (t !== "turn_end" && t !== "turn_report" && t !== "session_closed") return;
      if (seen) return;
      seen = true;
      sub.dispose();
      settle?.();
    });
    try {
      await this.agent.send(id, prompt);
    } catch (e) {
      // 发送失败（如 session_not_found）时释放监听，避免悬挂在已失效的会话上
      sub.dispose();
      throw e;
    }
    await ended;
    const frames = await this.agent.getMessages(id);
    const added = frames.filter(isAssistant).slice(before);
    // 取最后一条：本轮多步（工具调用）时最后一条才是最终答复
    return added.length ? textBlocks((added[added.length - 1] as Record<string, unknown>).message).trim() : "";
  }

  /** 用户手动裁决升级审批。 */
  async resolveApproval(workerId: string, requestId: string, decision: "approve" | "deny") {
    const esc = this._escalations.find((e) => e.workerId === workerId && e.item.requestId === requestId);
    if (!esc) return;
    try {
      if (esc.item.kind === "plan_review") {
        await this.agent.replyPlanReview(workerId, requestId, decision === "approve" ? "approve" : "keep_planning");
      } else {
        await this.agent.replyApproval(workerId, requestId, decision === "approve" ? "approve" : "deny");
      }
    } finally {
      this._escalations = this._escalations.filter((e) => !(e.workerId === workerId && e.item.requestId === requestId));
      this.persistEscalations();
      this.pushNotice({ role: "butler", kind: "text", text: `你已${decision === "approve" ? "批准" : "拒绝"}审批（任务「${esc.sessionTitle}」）。` });
      this.notify();
    }
  }

  async updateSettings(patch: Partial<ButlerSettings>) {
    this.settings = { ...this.settings, ...patch };
    this.persistSettings();
    this.notify();
  }

  async generateMorning() {
    const running = this.workers.filter((w) => w.status === "running").length;
    const done = this.workers.filter((w) => w.status === "done").length;
    const pending = this._escalations.length;
    this.pushNotice({
      role: "butler", kind: "morning", text: "",
      summary: [`进行中会话 ${running} 个`, `已完成 ${done} 个`, `待你裁决 ${pending} 项`],
    });
    this.notify();
  }
}

/** React 桥：订阅已创建的 ButlerEngine（单例由 setup 创建并 start，组件 props 下传）。 */
export function useButler(engine: ButlerEngine) {
  const [snapshot, setSnapshot] = useState<ButlerSnapshot>(() => ({
    ready: false, busy: false, phase: "", turnStartedMs: 0,
    settings: DEFAULT_SETTINGS, conversation: [], workers: [], escalations: [],
  }));

  useEffect(() => engine.subscribe(setSnapshot), [engine]);

  const delegate = useCallback((task: string) => engine.delegate(task), [engine]);
  const retry = useCallback((text: string) => engine.retry(text), [engine]);
  const resolveApproval = useCallback(
    (workerId: string, requestId: string, decision: "approve" | "deny") =>
      engine.resolveApproval(workerId, requestId, decision),
    [engine],
  );
  const updateSettings = useCallback((patch: Partial<ButlerSettings>) => engine.updateSettings(patch), [engine]);
  const generateMorning = useCallback(() => engine.generateMorning(), [engine]);

  return { snapshot, delegate, retry, resolveApproval, updateSettings, generateMorning };
}
