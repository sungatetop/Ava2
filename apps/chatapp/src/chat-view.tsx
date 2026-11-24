// chatapp 对话视图：经 ctx.agent（AgentChannel）走真实 onion 会话契约。
// 视觉与结构：列宽受限的消息流（用户气泡 / 助手无气泡 markdown）、
// 折叠的轮次过程区、hover 显示的操作行、圆角 composer。
import { Fragment, useEffect, useRef, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent } from "react";
import type {
  AgentChannel,
  AgentEventFrame,
  AnswerInput,
  AppContext,
  Disposable,
  GoalAction,
  GoalBlockedReason,
  GoalInfo,
  MessageAttachment,
  PendingItem,
  PlanInfo,
  ProviderInfo,
  SessionInfo,
  TodoItem,
} from "@ava-ai/app-sdk";
import { ApprovalChip, ModeChip, ModelChip, WorkspaceChip } from "./composer-chips";
import type { ApprovalPreset, ChatMode } from "./composer-chips";
import { applyFrame, todosFromEvent, turnReportFromFrame } from "./frame-reducer";
import type { ChatItem, PendingCard, TurnReport } from "./chat-types";
import { formatTokens } from "./format";
import { ApprovalCardView, ChatItems, Icon, PlanReviewCardView, QuestionCardView } from "./items";
import { chatappCss } from "./styles";
import { TrajectoryView } from "./trajectory";
import { WorkspacePicker } from "./workspace-picker";
import { chatStore, sessionLabel, useChatStore } from "./session-store";
import { downloadText, sessionToMarkdown } from "./export";

interface ChatViewProps {
  agent: AgentChannel;
  storage: AppContext["storage"];
  /** 原生系统文件夹选择器（桌面端 ctx.desktop.pickFolder；浏览器端缺省） */
  pickNative?: () => Promise<string | null>;
}

function errText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

// ---- 附件（图片） ----

/** 单张图片上限：base64 会随 user_message 落盘进 transcript，过大既撑请求又胀日志 */
const MAX_IMAGE_BYTES = 4 * 1024 * 1024;
/** 单条消息附件数上限 */
const MAX_ATTACHMENTS = 6;

/** File → MessageAttachment（data URL）。服务端把 `file` part 统一映射为 Image 内容块，
 *  故此处只接受图片；其余类型明确拒绝，避免以图片 URL 形式误发给模型。 */
function readImageAttachment(file: File): Promise<MessageAttachment> {
  return new Promise((resolve, reject) => {
    if (!file.type.startsWith("image/")) {
      reject(new Error(`仅支持图片附件：${file.name}`));
      return;
    }
    if (file.size > MAX_IMAGE_BYTES) {
      reject(new Error(`图片超过 4MB：${file.name}`));
      return;
    }
    const reader = new FileReader();
    reader.onload = () =>
      resolve({ name: file.name, mimeType: file.type, dataUrl: String(reader.result ?? "") });
    reader.onerror = () => reject(new Error(`读取失败：${file.name}`));
    reader.readAsDataURL(file);
  });
}

// ---- goal / plan / todos 展示辅助 ----

/** 本文件用到的图标路径（24×24 线性图标，与 items.tsx 的 Icon 同规格） */
const ICON = {
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
  terminal: "M4 5h16v14H4z M8 10l2 2-2 2 M13 14h3",
} as const;

const GOAL_PHASES: Record<string, string> = {
  active: "进行中",
  paused: "已暂停",
  blocked: "受阻",
  complete: "已完成",
};

const GOAL_ACTION_LABELS: Record<string, string> = {
  pause: "已暂停",
  resume: "已恢复",
  complete: "已完成",
  block: "已标记受阻",
  clear: "已清空",
};

const APPROVAL_PRESETS = ["workspace-write", "auto", "danger-full-access"] as const;

function phaseLabel(phase: string): string {
  return GOAL_PHASES[phase] ?? phase;
}

/** durable goal_change 事件 → GoalInfo（onion snake_case 快照 → camelCase 归一） */
function goalFromEvent(event: Record<string, unknown> | undefined): GoalInfo | null {
  const snap = event?.snapshot as Record<string, unknown> | undefined;
  if (!snap || typeof snap.id !== "string" || typeof snap.objective !== "string") return null;
  const reason = (snap.blocked_reason ?? snap.blockedReason) as Record<string, unknown> | undefined;
  return {
    id: snap.id as string,
    revision: Number(snap.revision ?? 0),
    objective: snap.objective as string,
    phase: String(snap.phase ?? "active"),
    blockedReason:
      reason && typeof reason === "object"
        ? ({ code: String(reason.code ?? ""), message: String(reason.message ?? "") } as GoalBlockedReason)
        : undefined,
    maxGoalRounds: Number(snap.max_goal_rounds ?? snap.maxGoalRounds ?? 0),
    roundsStarted: Number(snap.rounds_started ?? snap.roundsStarted ?? 0),
    createdAtMs: Number(snap.created_at_ms ?? snap.createdAtMs ?? 0),
    updatedAtMs: Number(snap.updated_at_ms ?? snap.updatedAtMs ?? 0),
    armed: snap.armed === true,
  };
}

/** durable plan_mode / plan_wanted 事件 → PlanInfo */
function planFromEvent(event: Record<string, unknown> | undefined): PlanInfo {
  return { active: event?.active === true, pending: null };
}

// ---- 斜杠命令 ----

/** 斜杠命令清单（分组 + 图标 + 中文标签 + 命令名 + 描述）。
 *  只列没有等价操作面的命令：新建会话、中断、切换模型、切换审批都有常驻控件，不再重复进菜单。 */
interface SlashCommand {
  name: string;
  /** 参数提示（无参数的命令选中即执行） */
  args?: string;
  /** 中文标签 */
  label: string;
  desc: string;
  /** 分组：add（添加）/ commands（指令） */
  section: "add" | "commands";
  icon: string;
}

const SLASH_COMMANDS: SlashCommand[] = [
  { name: "goal", args: "[<目标>|clear|edit <目标>|pause|resume]", label: "目标", desc: "设置或查看长期任务目标", section: "add", icon: ICON.target },
  { name: "plan", args: "[off|消息]", label: "计划", desc: "进入或退出计划模式", section: "add", icon: ICON.plan },
  { name: "compact", label: "压缩", desc: "压缩以上对话内容", section: "commands", icon: ICON.terminal },
  { name: "permission", args: "[<预设>]", label: "权限", desc: "查看或切换审批预设", section: "commands", icon: ICON.terminal },
  { name: "feedback", args: "<文本>", label: "反馈", desc: "记录一条会话反馈", section: "commands", icon: ICON.terminal },
  { name: "export", label: "导出", desc: "把本会话导出为 Markdown 文件", section: "commands", icon: ICON.terminal },
];

const SECTION_LABELS: Record<SlashCommand["section"], string> = {
  add: "添加",
  commands: "指令",
};

/** 中文别名 → 规范命令名。
 *  宿主命令由服务端解析别名，这里只为本地命令与面板回显做同一套映射。 */
const COMMAND_ALIASES: Record<string, string> = {
  目标: "goal",
  计划: "plan",
  反馈: "feedback",
  压缩: "compact",
  权限: "permission",
  导出: "export",
};

const HELP_TEXT = [
  "/compact — 压缩上下文（别名 /压缩）",
  "/goal <目标> — 创建/编辑目标；/goal pause|resume|complete|block|clear 操作（别名 /目标）",
  "/plan [off|消息] — 进入 plan 模式（可带引导消息）；/plan off 退出（别名 /计划）",
  "/permission [<预设>] — 查看或切换审批预设（workspace-write / auto / danger-full-access）（别名 /权限）",
  "/feedback <文本> — 记录一条会话反馈（别名 /反馈）",
  "/export — 把本会话导出为 Markdown 文件（别名 /导出）",
].join("\n");

/** 会话目标横幅：composer 上方独立卡片，complete 不渲染。
 *  phase 文案带「目标」后缀；动作顺序 edit / clear / pause|resume；
 *  编辑为条内内联输入，不占用 composer。 */
function GoalBar({
  goal,
  onAction,
  onEdit,
}: {
  goal: GoalInfo;
  onAction: (action: GoalAction) => void;
  onEdit: (objective: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const phase =
    goal.phase === "active"
      ? goal.armed
        ? "进行中的目标"
        : "待继续的目标"
      : goal.phase === "paused"
        ? "已暂停的目标"
        : goal.phase === "blocked"
          ? "受阻的目标"
          : "进行中的目标";
  const blocked = goal.phase === "blocked" ? goal.blockedReason?.message : undefined;
  // active 且 armed → pause，否则（暂停 / 解除武装 / 受阻）→ resume
  const resumable = goal.phase !== "active" || !goal.armed;
  const save = () => {
    const trimmed = draft.trim();
    if (trimmed === "") return;
    onEdit(trimmed);
    setEditing(false);
  };
  if (editing) {
    return (
      <div className={"cc-goalbar is-" + goal.phase}>
        <span className="cc-goal-glyph">
          <Icon d={ICON.target} size={14} />
        </span>
        <input
          className="cc-goal-input"
          value={draft}
          autoFocus
          aria-label="目标内容"
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              save();
            } else if (e.key === "Escape") {
              e.preventDefault();
              setEditing(false);
            }
          }}
        />
        <span className="cc-goal-actions">
          <button className="cc-goal-btn" onClick={save} disabled={draft.trim() === ""} title="保存">
            <Icon d={ICON.check} size={14} />
          </button>
          <button className="cc-goal-btn" onClick={() => setEditing(false)} title="取消">
            <Icon d={ICON.close} size={14} />
          </button>
        </span>
      </div>
    );
  }
  return (
    <div
      className={"cc-goalbar is-" + goal.phase}
      title={blocked || `${goal.objective}（第 ${goal.roundsStarted}/${goal.maxGoalRounds} 轮）`}
    >
      <span className="cc-goal-glyph">
        <Icon d={ICON.target} size={14} />
      </span>
      <span className="cc-goal-label">{phase}</span>
      <span className="cc-goal-text">{goal.objective}</span>
      <span className="cc-goal-actions">
        <button
          className="cc-goal-btn"
          onClick={() => {
            setDraft(goal.objective);
            setEditing(true);
          }}
          title="编辑目标"
        >
          <Icon d={ICON.edit} size={14} />
        </button>
        <button className="cc-goal-btn" onClick={() => onAction("clear")} title="清除目标">
          <Icon d={ICON.trash} size={14} />
        </button>
        {resumable ? (
          <button className="cc-goal-btn" onClick={() => onAction("resume")} title="继续目标">
            <Icon d={ICON.resume} size={14} />
          </button>
        ) : (
          <button className="cc-goal-btn" onClick={() => onAction("pause")} title="暂停目标">
            <Icon d={ICON.pause} size={14} />
          </button>
        )}
      </span>
    </div>
  );
}

/** 待办状态点：completed 实心点、in_progress 旋转环、pending 中性点 */
function TodoStateDot({ status }: { status: string }) {
  if (status === "in_progress") {
    return (
      <span className="cc-statedot is-ongoing" aria-label="进行中">
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <circle className="cc-statedot-track" cx="8" cy="8" r="6" />
          <circle className="cc-statedot-arc" cx="8" cy="8" r="6" />
        </svg>
      </span>
    );
  }
  return <span className={"cc-statedot " + (status === "completed" ? "is-done" : "is-idle")} aria-hidden="true" />;
}

/** 待办进度文案：只列出非零分段，en space 分隔 */
function todoProgress(todos: TodoItem[]): string {
  const done = todos.filter((t) => t.status === "completed").length;
  const active = todos.filter((t) => t.status === "in_progress").length;
  const pending = todos.length - done - active;
  return [
    done > 0 ? `${done} 已完成` : "",
    active > 0 ? `${active} 进行中` : "",
    pending > 0 ? `${pending} 待处理` : "",
  ]
    .filter(Boolean)
    .join("\u2002·\u2002");
}

/** 待办面板：composer 上方独立卡片，空清单不占位、默认折叠；
 *  折叠头显示「任务 + 进度」，展开后逐项列状态点与内容。 */
function TodoPanel({ todos }: { todos: TodoItem[] }) {
  const [collapsed, setCollapsed] = useState(true);
  if (todos.length === 0) return null;
  return (
    <div className="cc-todopanel">
      <div className="cc-todopanel-body">
        <button className="cc-todopanel-head" onClick={() => setCollapsed((v) => !v)} aria-expanded={!collapsed}>
          <span className="cc-todopanel-glyph">
            <Icon d={ICON.checklist} size={14} />
          </span>
          <span className="cc-todopanel-title">任务</span>
          <span className="cc-todopanel-progress">{todoProgress(todos)}</span>
          <span className={"cc-chevron"}>
            <Icon d={collapsed ? ICON.chevronUp : ICON.chevron} />
          </span>
        </button>
        {!collapsed && (
          <ul className="cc-todopanel-list">
            {todos.map((t, i) => (
              <li className="cc-todopanel-item" key={i} data-status={t.status}>
                <span className="cc-todopanel-dot">
                  <TodoStateDot status={t.status} />
                </span>
                <span className="cc-todopanel-text">{t.content}</span>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

/** plan 模式徽标（composer 工具条内，点击即退出 plan 模式；
 *  pending 表示切换待生效——有效目标是 `pending ? !active : active`）。 */
function PlanChip({ plan, onExit }: { plan: PlanInfo; onExit: () => void }) {
  const effective = plan.pending !== null && plan.pending !== undefined ? !plan.active : plan.active;
  if (!effective) return null;
  return (
    <button
      className={"cc-planchip" + (plan.pending !== null && plan.pending !== undefined ? " is-pending" : "")}
      onClick={onExit}
      title={plan.pending !== null && plan.pending !== undefined ? "plan 模式切换中（当前轮次结束后生效）" : "点击退出 plan 模式"}
    >
      <span className="cc-planchip-ico">
        <Icon d={ICON.plan} size={14} />
      </span>
      <span className="cc-planchip-close">
        <Icon d={ICON.closeCircle} size={14} />
      </span>
      计划模式
    </button>
  );
}

/** question_requested 帧的问题列表（onion snake_case）→ camelCase 归一 */
function normalizeQuestions(raw: unknown): unknown[] {
  const list = Array.isArray(raw) ? (raw as unknown[]) : [];
  return list.map((q) => {
    const obj = (typeof q === "object" && q !== null ? q : {}) as Record<string, unknown>;
    return {
      id: obj.id,
      question: obj.question,
      header: obj.header,
      options: obj.options,
      multiSelect: obj.multiSelect === true || obj.multi_select === true,
    };
  });
}

/** SessionNotice 待回填帧 → PendingCard */
function pendingFromFrame(frame: AgentEventFrame): PendingCard | null {
  const requestId = typeof frame.requestId === "string" ? frame.requestId : "";
  if (!requestId) return null;
  switch (frame.type) {
    case "approval_requested":
      return {
        requestId,
        kind: "approval",
        tool: frame.tool as string | undefined,
        input: frame.input,
        reason: frame.reason as string | undefined,
      };
    case "plan_review_requested":
      return { requestId, kind: "plan_review", plan: frame.plan as string | undefined };
    case "question_requested":
      return {
        requestId,
        kind: "question",
        callId: frame.callId as string | undefined,
        questions: normalizeQuestions(frame.questions),
      };
    default:
      return null;
  }
}

/** REST PendingItem（getPending 存量）→ PendingCard */
function pendingFromRest(item: PendingItem): PendingCard {
  return {
    requestId: item.requestId,
    kind: item.kind,
    tool: item.tool,
    input: item.input,
    reason: item.reason,
    plan: item.plan,
    callId: item.callId,
    questions: item.questions,
  };
}

/** 上下文占比：估算占用 / 窗口上限（≥70% 警告、≥90% 危险着色） */
function ContextMeter({ tokens, limit }: { tokens: number; limit: number }) {
  const pct = limit > 0 ? Math.min(100, Math.round((tokens / limit) * 100)) : 0;
  const level = pct >= 90 ? " danger" : pct >= 70 ? " warn" : "";
  return (
    <div
      className={"cc-ctx" + level}
      title={`上下文占用 ${tokens.toLocaleString()} / ${limit.toLocaleString()} tokens（chars/4 估算）`}
    >
      <span className="cc-ctx-bar">
        <i style={{ width: `${pct}%` }} />
      </span>
      <span className="cc-ctx-text">
        上下文 {pct}%{limit > 0 ? ` · ${formatTokens(tokens)}/${formatTokens(limit)}` : ""}
      </span>
    </div>
  );
}

export function ChatView({ agent, storage, pickNative }: ChatViewProps) {
  const [items, setItems] = useState<ChatItem[]>([]);
  const [pending, setPending] = useState<PendingCard[]>([]);
  const [input, setInput] = useState("");
  /** 待发送的图片附件（发送成功后清空；图片是当前服务端唯一支持的附件类型） */
  const [attachments, setAttachments] = useState<MessageAttachment[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** 会话列表 / 当前会话 / 近用工作区：与侧边栏面板共享（session-store） */
  const { currentId: sessionId, recents: workspaces, command } = useChatStore();
  /** 当前会话详情（模型 / 审批预设 / 工作区读面） */
  const [info, setInfo] = useState<SessionInfo | null>(null);
  /** 模型目录（providers 分组，含各 provider 模型列表） */
  const [catalog, setCatalog] = useState<ProviderInfo[]>([]);
  /** 工作目录选择器开关（以选择目录代替手输路径） */
  const [wsPicker, setWsPicker] = useState(false);
  /** 最近轮次摘要（live turn_report 帧 / REST lastTurn），用于补齐快照重放的轮次头 */
  const [lastTurn, setLastTurn] = useState<TurnReport | null>(null);
  /** 中栏视图：对话（消息流）/ 轨迹（派生记录表，参考 ava_square TrajectoryView） */
  const [view, setView] = useState<"chat" | "trajectory">("chat");
  // 暂存模式（作用于下一次新建会话，创建后即弃）
  const [mode, setMode] = useState<ChatMode>("standard");
  /** 草稿态（点了新建但还没发首条消息）：工作区与首选模型，发送时才真正创建会话 */
  const [draftCwd, setDraftCwd] = useState<string | undefined>(undefined);
  const [draftModel, setDraftModel] = useState("");
  /** 会话目标（durable goal_change / REST 同步；存在时 composer 上方显示横幅） */
  const [goal, setGoal] = useState<GoalInfo | null>(null);
  /** plan 模式（durable plan_mode/plan_wanted / REST 同步；pending 表示切换待生效） */
  const [plan, setPlan] = useState<PlanInfo | null>(null);
  /** 当前待办（durable todo_write / REST 同步；与消息流待办卡片同源） */
  const [todos, setTodos] = useState<TodoItem[]>([]);
  /** 斜杠菜单选中项（↑↓ 移动）与 Escape 暂时关闭 */
  const [slashIndex, setSlashIndex] = useState(0);
  const [slashDismissed, setSlashDismissed] = useState(false);
  const busyRef = useRef(false);
  const sessionRef = useRef<string | null>(null);
  const subRef = useRef<Disposable | null>(null);
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLTextAreaElement | null>(null);
  const fileRef = useRef<HTMLInputElement | null>(null);
  /** 斜杠菜单滚动视口（listbox 位于滚动视口，键盘移动时 scrollIntoView） */
  const slashListRef = useRef<HTMLDivElement | null>(null);

  /** 追加图片附件（选择器 / 粘贴 / 拖放共用）；失败项以 error 汇总提示 */
  const addFiles = async (files: FileList | File[]) => {
    const next: MessageAttachment[] = [];
    const errs: string[] = [];
    for (const file of Array.from(files)) {
      try {
        next.push(await readImageAttachment(file));
      } catch (e) {
        errs.push(errText(e));
      }
    }
    if (next.length > 0) {
      // 上限判断放在 updater 外：updater 必须是纯函数（StrictMode 会重复执行）
      if (attachments.length + next.length > MAX_ATTACHMENTS) {
        errs.push(`单条消息最多 ${MAX_ATTACHMENTS} 张图片`);
      }
      setAttachments((prev) => [...prev, ...next].slice(0, MAX_ATTACHMENTS));
    }
    if (errs.length > 0) setError(errs.join("；"));
  };

  // 新条目 / 卡片到达时滚到底部
  useEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [items, pending]);

  // 快照重放不含 turn_report（live 帧不落盘）：用 REST lastTurn 补齐末尾收束轮次的过程头
  useEffect(() => {
    if (!lastTurn) return;
    setItems((prev) => {
      const last = prev[prev.length - 1];
      if (!last || last.kind !== "turn" || last.streaming || last.report) return prev;
      if (last.turn !== lastTurn.turn) return prev;
      return [...prev.slice(0, -1), { ...last, report: lastTurn }];
    });
  }, [items, lastTurn]);

  const handleFrame = (frame: AgentEventFrame) => {
    // 待回填帧 → 卡片区（requestId 去重）
    const card = pendingFromFrame(frame);
    if (card) {
      setPending((prev) =>
        prev.some((p) => p.requestId === card.requestId) ? prev : [...prev, card],
      );
      return;
    }
    if (frame.type === "session_closed") {
      busyRef.current = false;
      setBusy(false);
      return;
    }
    if (frame.type === "durable") {
      const event = frame.event as Record<string, unknown> | undefined;
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
          // durable 快照不含 armed（激活是进程内状态，不落盘）→ 以 REST 读面为准回填，
          // 否则新建/续轮目标会被误显示成「待继续的目标」。
          const id = sessionRef.current;
          if (next && id) {
            agent
              .getGoal(id)
              .then((fresh) => {
                if (sessionRef.current === id) setGoal(fresh);
              })
              .catch(() => undefined);
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
      // 轮次收束即生成本轮答疑结束：解除 busy（否则会话在本轮结束后一直卡在「生成中…」）
      busyRef.current = false;
      setBusy(false);
      setLastTurn(turnReportFromFrame(frame));
      // 轮次结束：重新拉会话详情刷新上下文占用（durable 估算随本轮增长）
      const id = sessionRef.current;
      if (id) {
        agent
          .getSession(id)
          .then((loaded) => {
            if (sessionRef.current === id) setInfo(loaded);
          })
          .catch(() => undefined);
      }
    }
    // 渲染帧（含 turn_report → 轮次块过程头）→ 纯函数 reducer 聚合
    setItems((prev) => applyFrame(prev, frame));
  };

  const attach = (id: string) => {
    if (!id || sessionRef.current === id) return;
    subRef.current?.dispose();
    sessionRef.current = id;
    subRef.current = agent.onSessionEvent(id, handleFrame);
    chatStore.setCurrent(id);
    // 切换即重置视图：快照帧到达后由 reducer 重放历史
    setItems([]);
    setPending([]);
    busyRef.current = false;
    setBusy(false);
    setError(null);
    setLastTurn(null);
    setGoal(null);
    setPlan(null);
    setTodos([]);
    // 存量待回填拉取（重连 / 后进场景）+ 最近轮次摘要（补齐重放过程头）
    agent
      .getPending(id)
      .then((list) => setPending(list.map(pendingFromRest)))
      .catch(() => undefined);
    agent
      .getSession(id)
      .then((loaded) => {
        if (sessionRef.current !== id) return;
        setInfo(loaded);
        setLastTurn(loaded.lastTurn ?? null);
      })
      .catch(() => undefined);
    // goal / plan / 待办存量（重连 / 后进场景；live 期间由 durable 事件驱动）
    agent
      .getGoal(id)
      .then((g) => {
        if (sessionRef.current === id) setGoal(g);
      })
      .catch(() => undefined);
    agent
      .getPlan(id)
      .then((p) => {
        if (sessionRef.current === id) setPlan(p);
      })
      .catch(() => undefined);
    agent
      .getTodos(id)
      .then((list) => {
        if (sessionRef.current === id) setTodos(list);
      })
      .catch(() => undefined);
  };

  /** 会话列表刷新（侧边栏分组的唯一数据源） */
  const refreshSessions = async () => {
    try {
      chatStore.setSessions(await agent.listSessions());
    } catch {
      /* 列表刷新失败不打断对话 */
    }
  };

  // 默认工作区：用户主目录（`/v1/fs/dirs` 无参即回落到主目录）。
  // 不继承边车的进程工作目录——那是安装/源码目录，对终端用户不是「我的工作区」。
  const homeDirRef = useRef<string | null>(null);
  const defaultWorkspace = async (): Promise<string | undefined> => {
    if (homeDirRef.current) return homeDirRef.current;
    try {
      const listing = await agent.listDirs();
      homeDirRef.current = listing.path;
      return listing.path;
    } catch {
      return undefined;
    }
  };

  // 草稿 → 首条消息：此刻才真正创建会话（惰性创建），
  // 并套用草稿选定的工作区 / 模式 / 模型。
  const ensure = async (): Promise<string> => {
    const id = sessionRef.current;
    if (id) return id;
    const workspace = draftCwd ?? (await defaultWorkspace());
    const created = await agent.createSession(workspace, mode === "creator" ? "creator" : undefined);
    setMode("standard");
    setDraftCwd(undefined);
    attach(created.id);
    if (draftModel && created.model !== draftModel) {
      try {
        setInfo(await agent.setSessionModel(created.id, draftModel));
      } catch {
        /* 新会话沿用默认模型亦可 */
      }
    }
    return created.id;
  };

  // 挂载：拉会话列表并 attach 最近一个（attach 内部做快照重放与存量拉取）
  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const raw = await agent.listSessions();
        if (!alive) return;
        const list = chatStore.setSessions(raw);
        if (list.length > 0) attach(list[0].id);
      } catch (e) {
        if (alive) setError(`会话列表不可达：${errText(e)}`);
      }
    })();
    return () => {
      alive = false;
      subRef.current?.dispose();
    };
  }, [agent]);

  // 挂载：模型目录 + 近用工作区（storage 持久化）
  useEffect(() => {
    let alive = true;
    agent
      .listModels()
      .then((data) => {
        if (!alive) return;
        setCatalog(data.providers);
        // 草稿态没有会话详情：chip 先显示默认模型，创建会话时再按实际模型校正
        setDraftModel((prev) => prev || data.defaultModel);
      })
      .catch(() => undefined);
    storage
      .get<string[]>("recentWorkspaces")
      .then((list) => {
        if (alive && Array.isArray(list)) chatStore.setRecents(list);
      })
      .catch(() => undefined);
    // 上次选择的模型 → 草稿首选模型（首条消息创建会话时套用）
    storage
      .get<string>("lastModel")
      .then((model) => {
        if (alive && typeof model === "string") setDraftModel(model);
      })
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, [agent, storage]);

  /** 停止当前轮次（composer 的方形按钮 / `/stop` 同源）：只发中断请求，
   *  真实收束以 turn_report(reason=cancelled) 为准，故这里不立即解除 busy。 */
  const stopTurn = async () => {
    const id = sessionRef.current;
    if (!id) return;
    try {
      await agent.interrupt(id);
    } catch (e) {
      setError(`中断失败：${errText(e)}`);
      busyRef.current = false;
      setBusy(false);
    }
  };

  const send = async () => {
    const text = input.trim();
    const files = attachments;
    if (!text && files.length === 0) return;
    // 斜杠命令：本地执行，不建会话、不发 agent、不占用 busy（/stop 在生成中也必须可用）；
    // 带附件时按普通消息处理（命令不消费附件）
    if (text.startsWith("/") && files.length === 0) {
      await runSlash(text);
      return;
    }
    if (busyRef.current) return;
    busyRef.current = true; // 防重入：草稿创建会话期间也要挡住重复发送
    setError(null);
    setInput("");
    setAttachments([]);
    // 乐观气泡（durable user_message 回显后由 reducer 转正）
    setItems((prev) => [
      ...prev,
      { kind: "user", text, attachments: files.length > 0 ? files : undefined, optimistic: true },
    ]);
    try {
      // 草稿 → 会话（ensure 内部 attach 会重置视图状态，故 busy 的 UI 状态在其之后再置位）
      const id = await ensure();
      setBusy(true);
      await agent.send(id, text, files);
      // 会话此刻才真正出现在列表里（惰性创建：首条消息落盘后才刷新）
      void refreshSessions();
    } catch (e) {
      busyRef.current = false;
      setBusy(false);
      // 回滚乐观气泡（文本 + 附件数一致才回滚，避免误删上一条）
      setItems((prev) => {
        const last = prev[prev.length - 1];
        if (
          last?.kind === "user" &&
          last.optimistic &&
          last.text === text &&
          (last.attachments?.length ?? 0) === files.length
        ) {
          return prev.slice(0, -1);
        }
        return prev;
      });
      // 发送失败把附件放回输入区，避免用户重选
      if (files.length > 0) setAttachments(files);
      setError(`发送失败：${errText(e)}`);
    }
  };

  // 新建会话（可选 cwd = 切工作区）：只进入草稿态，**不落盘**——首条消息发送时才创建会话
  //（点新建只打开空白视图，未发消息的会话不进历史列表）。
  const newSession = async (cwd?: string) => {
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
    setDraftCwd(cwd);
    if (cwd) {
      const next = [cwd, ...workspaces.filter((w) => w !== cwd)].slice(0, 5);
      chatStore.setRecents(next);
      await storage.set("recentWorkspaces", next).catch(() => undefined);
    }
  };

  // 侧边栏命令（切会话 / 新建）：seq 去重，React 严格模式重复挂载时不重放
  const lastCommandSeq = useRef(0);
  useEffect(() => {
    if (!command || command.seq === lastCommandSeq.current) return;
    lastCommandSeq.current = command.seq;
    if (command.kind === "select") attach(command.id);
    else void newSession(command.cwd);
  }, [command]);

  /** 命令结果（success / error + 文本） */
  type CommandOutcome = { kind: "success" | "error"; text: string };
  const ok = (text: string): CommandOutcome => ({ kind: "success", text });
  const fail = (text: string): CommandOutcome => ({ kind: "error", text });

  /** 斜杠命令分发。
   *  - goal / plan / compact 由宿主执行（POST /commands），命令行与结果落 durable
   *    `command_run` / `command_done`，刷新后仍在时间线；
   *  - help / new / stop / model / approval 是 chatapp 本地能力，就地合成同形态条目。 */
  const runSlash = async (raw: string) => {
    const m = /^\/(\S+)(?:\s+([\s\S]*))?$/.exec(raw.trim());
    if (!m) return;
    const typed = m[1].toLowerCase();
    // 中文别名归一：`/压缩` 等同 `/compact`。
    const name = COMMAND_ALIASES[typed] ?? typed;
    const arg = (m[2] ?? "").trim();
    const id = sessionRef.current;
    setInput("");

    // 宿主命令：事件流负责渲染，这里只发请求
    if (
      name === "goal" ||
      name === "plan" ||
      name === "compact" ||
      name === "feedback" ||
      name === "permission"
    ) {
      if (!id) {
        setItems((prev) => [
          ...prev,
          { kind: "notice", tone: "error", text: `无会话，无法执行 /${name}` },
        ]);
        return;
      }
      try {
        await agent.runCommand(id, name, arg);
      } catch (e) {
        setError(`/${name} 执行失败：${errText(e)}`);
      }
      return;
    }

    // 本地命令：回显气泡 + 执行中结果行（终态就地回填）
    const itemId = `cmd-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    setItems((prev) => [
      ...prev,
      { kind: "command", id: itemId, name, args: arg ? " " + arg : "", outcome: null },
    ]);
    const settle = (outcome: CommandOutcome) =>
      setItems((prev) =>
        prev.map((it) => (it.kind === "command" && it.id === itemId ? { ...it, outcome } : it)),
      );
    try {
      switch (name) {
        case "help":
          settle(ok(HELP_TEXT));
          return;
        case "new":
          await newSession(arg || undefined);
          settle(ok(arg ? `已切到新会话草稿（工作区 ${arg}）` : "已开始新会话（发送首条消息后创建）"));
          return;
        case "stop":
          if (!id) {
            settle(fail("无会话可中断"));
            return;
          }
          await agent.interrupt(id);
          settle(ok("已请求中断当前轮次"));
          return;
        case "model": {
          if (!id) {
            settle(fail("无会话可切换模型"));
            return;
          }
          if (!arg) {
            settle(ok(`当前模型：${info?.model ?? "默认"}`));
            return;
          }
          setInfo(await agent.setSessionModel(id, arg));
          await storage.set("lastModel", arg);
          settle(ok(`模型已切换为 ${arg}`));
          return;
        }
        case "approval": {
          if (!id) {
            settle(fail("无会话可切换审批预设"));
            return;
          }
          if (!(APPROVAL_PRESETS as readonly string[]).includes(arg)) {
            settle(fail(`未知审批预设：${arg || "（空）"}（可用：${APPROVAL_PRESETS.join(" / ")}）`));
            return;
          }
          setInfo(await agent.setApprovalPreset(id, arg as ApprovalPreset));
          settle(ok(`审批预设已切换为 ${arg}`));
          return;
        }
        case "export": {
          // `/export` 不接受路径参数，直接产出本会话 Markdown 下载。
          if (!id) {
            settle(fail("无会话可导出"));
            return;
          }
          if (arg) {
            settle(fail("/export 不接受路径参数"));
            return;
          }
          const session = chatStore.snapshot().sessions.find((item) => item.id === id);
          if (!session) {
            settle(fail("会话信息尚未加载，稍后重试"));
            return;
          }
          downloadText(sessionLabel(session), await sessionToMarkdown(agent, session));
          settle(ok("已导出会话 Markdown"));
          return;
        }
        default:
          settle(fail(`未知命令 /${name}（输入 /help 查看命令列表）`));
      }
    } catch (e) {
      settle(fail(`/${name} 执行失败：${errText(e)}`));
    }
  };


  const onModelChange = async (model: string) => {
    if (!model) return;
    const id = sessionRef.current;
    if (!id) {
      // 草稿态（尚未创建会话）：只记偏好，首条消息创建会话时套用
      setDraftModel(model);
      await storage.set("lastModel", model).catch(() => undefined);
      return;
    }
    try {
      setInfo(await agent.setSessionModel(id, model));
      await storage.set("lastModel", model);
      setDraftModel(model);
    } catch (e) {
      setError(`模型切换失败：${errText(e)}`);
    }
  };

  const onApprovalChange = async (preset: ApprovalPreset) => {
    const id = sessionRef.current;
    if (!id) return;
    try {
      setInfo(await agent.setApprovalPreset(id, preset));
    } catch (e) {
      setError(`审批模式切换失败：${errText(e)}`);
    }
  };

  const removePending = (requestId: string) => {
    setPending((prev) => prev.filter((p) => p.requestId !== requestId));
  };

  /** GoalBar 动作（pause / resume / clear）：直接走 agent，不经过斜杠命令 */
  const onGoalAction = async (action: GoalAction) => {
    const id = sessionRef.current;
    if (!id) return;
    try {
      const next = await agent.goalAction(id, action, goal?.revision ?? 0);
      setGoal(next);
    } catch (e) {
      setError(`目标操作失败：${errText(e)}`);
    }
  };

  /** GoalBar 编辑：条内内联输入直接提交，不占用 composer */
  const onGoalEdit = async (objective: string) => {
    const id = sessionRef.current;
    if (!id) return;
    try {
      setGoal(await agent.setGoal(id, { objective, ...(goal ? { revision: goal.revision } : {}) }));
    } catch (e) {
      setError(`目标更新失败：${errText(e)}`);
    }
  };

  /** PlanChip 点击 = 退出 plan 模式（等同 `/plan off`） */
  const onExitPlan = async () => {
    const id = sessionRef.current;
    if (!id) return;
    try {
      setPlan(await agent.setPlan(id, false));
    } catch (e) {
      setError(`退出 plan 模式失败：${errText(e)}`);
    }
  };

  const onDecision = async (
    requestId: string,
    decision: "approve" | "approve_for_session" | "deny",
  ) => {
    const id = sessionRef.current;
    if (!id) return;
    try {
      await agent.replyApproval(id, requestId, decision);
      removePending(requestId);
    } catch (e) {
      setError(`审批回填失败：${errText(e)}`);
    }
  };

  const onVerdict = async (requestId: string, verdict: "approve" | "keep_planning" | "dismissed") => {
    const id = sessionRef.current;
    if (!id) return;
    try {
      await agent.replyPlanReview(id, requestId, verdict);
      removePending(requestId);
    } catch (e) {
      setError(`计划评审回填失败：${errText(e)}`);
    }
  };

  const onSubmitAnswers = async (requestId: string, answers: AnswerInput[]) => {
    const id = sessionRef.current;
    if (!id) return;
    try {
      await agent.answerQuestion(id, requestId, answers);
      removePending(requestId);
    } catch (e) {
      setError(`问询答复失败：${errText(e)}`);
    }
  };

  // 斜杠菜单派生态：输入为 "/" + 命令词（尚无空格参数）时显示；过滤用前缀匹配
  const slashMatch = /^\/(\S*)$/.exec(input);
  const slashMenuOpen = slashMatch !== null && !busy;
  const slashQuery = (slashMatch?.[1] ?? "").toLowerCase();
  const slashFiltered = slashMenuOpen
    ? SLASH_COMMANDS.filter((c) => ("/" + c.name).startsWith("/" + slashQuery))
    : [];
  const slashActive = Math.min(slashIndex, Math.max(slashFiltered.length - 1, 0));
  /** 菜单滚动视口是否还有下方未显示内容（溢出时底部渐变提示，滚到底消失） */
  const [slashOverflowBelow, setSlashOverflowBelow] = useState(false);
  useEffect(() => {
    if (!slashMenuOpen || slashDismissed) return;
    const viewport = slashListRef.current;
    setSlashOverflowBelow(viewport !== null && viewport.scrollTop + viewport.clientHeight < viewport.scrollHeight - 1);
  });
  // 焦点留在 textarea（combobox 模式），浏览器不会替键盘导航滚动选项——这里手动滚入视图
  useEffect(() => {
    if (!slashMenuOpen || slashDismissed || slashFiltered.length === 0) return;
    document.getElementById(`cc-slash-option-${slashActive}`)?.scrollIntoView({ block: "nearest" });
  }, [slashMenuOpen, slashDismissed, slashActive, slashFiltered.length]);

  const onKeyDown = (e: ReactKeyboardEvent<HTMLTextAreaElement>) => {
    // 斜杠菜单键盘导航：↑↓ 移动、Tab/Enter 选中（带参命令填充占位，无参命令直接执行）、Esc 暂时收起
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
        // 带参命令：Tab 补全为 "/name "；Enter 仅在输入恰为完整命令名时直接执行
        //（支持 /goal、/plan 的无参查看），否则填充参数占位。
        if (cmd.args && !(e.key === "Enter" && input.trim() === "/" + cmd.name)) {
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
  const canSend = !busy && (input.trim() !== "" || attachments.length > 0);

  return (
    <div className="cc-root">
      <style>{chatappCss}</style>

      <header className="cc-head">
        <div className="cc-viewtabs" role="tablist" aria-label="中栏视图">
          <button
            className={"cc-viewtab" + (view === "chat" ? " on" : "")}
            role="tab"
            aria-selected={view === "chat"}
            onClick={() => setView("chat")}
          >
            对话
          </button>
          <button
            className={"cc-viewtab" + (view === "trajectory" ? " on" : "")}
            role="tab"
            aria-selected={view === "trajectory"}
            onClick={() => setView("trajectory")}
          >
            轨迹
          </button>
        </div>
        <div className="cc-head-right">
          {busy && <span className="cc-head-dim">生成中…</span>}
          <ContextMeter
            tokens={info?.contextTokens ?? 0}
            limit={info?.contextWindowTokens ?? 0}
          />
        </div>
      </header>

      <div className="cc-main">
        {view === "trajectory" ? (
          <TrajectoryView items={items} />
        ) : (
          <div className="cc-scroll" ref={scrollRef}>
            {items.length === 0 && pending.length === 0 ? (
              <div className="cc-empty">
                <span className="cc-empty-ico">
                  <svg
                    width="20"
                    height="20"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="1.8"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <path d="M7.9 20A9 9 0 1 0 4 16.1L2 22Z" />
                  </svg>
                </span>
                <span className="cc-empty-title">开始新的对话</span>
                <span>在下方输入消息，Agent 会即时响应。</span>
              </div>
            ) : (
              <div className="cc-flow">
                <ChatItems items={items} sessionId={sessionId ?? undefined} />
                {pending.map((card) =>
                  card.kind === "approval" ? (
                    <ApprovalCardView
                      key={card.requestId}
                      card={card}
                      onDecision={(id, d) => void onDecision(id, d)}
                    />
                  ) : card.kind === "plan_review" ? (
                    <PlanReviewCardView
                      key={card.requestId}
                      card={card}
                      onVerdict={(id, v) => void onVerdict(id, v)}
                    />
                  ) : (
                    <QuestionCardView
                      key={card.requestId}
                      card={card}
                      onSubmit={(id, a) => void onSubmitAnswers(id, a)}
                    />
                  ),
                )}
              </div>
            )}
          </div>
        )}
      </div>

      {error && <div className="cc-error">{error}</div>}

      {/* 轨迹视图面向「读过程」：隐藏续派输入区（对齐 ava_square §9.1），对话视图保留 */}
      {view === "chat" && (
        <div className="cc-composer-wrap">
        {/* composer 上方 dock 栈：待办面板（order 0）在上、目标横幅（order 10）在下 */}
        <TodoPanel todos={todos} />
        {goal && goal.phase !== "complete" && (
          <GoalBar goal={goal} onAction={(action) => void onGoalAction(action)} onEdit={(o) => void onGoalEdit(o)} />
        )}
        <div className="cc-composer">
          {/* 斜杠命令菜单：combobox 模式——焦点留在 textarea，
              listbox 挂在滚动视口；无匹配时不渲染浮层，Enter 按普通消息发送 */}
          {slashMenuOpen && !slashDismissed && slashFiltered.length > 0 && (
            <div className={"cc-slash" + (slashOverflowBelow ? " is-overflow-below" : "")}>
              <div
                ref={slashListRef}
                className="cc-slash-list"
                role="listbox"
                aria-label="命令建议"
                aria-activedescendant={`cc-slash-option-${slashActive}`}
                onScroll={() => {
                  const vp = slashListRef.current;
                  if (vp) setSlashOverflowBelow(vp.scrollTop + vp.clientHeight < vp.scrollHeight - 1);
                }}
              >
                {slashFiltered.map((c, i) => {
                  // 分组标题：仅在分组首行前渲染（add 组 / commands 组）
                  const prev = slashFiltered[i - 1];
                  return (
                    <Fragment key={c.name}>
                      {!prev || prev.section !== c.section ? (
                        <div className="cc-slash-section">{SECTION_LABELS[c.section]}</div>
                      ) : null}
                      <button
                        id={`cc-slash-option-${i}`}
                        type="button"
                        role="option"
                        aria-selected={i === slashActive}
                        className={"cc-slash-item" + (i === slashActive ? " on" : "")}
                        onMouseDown={(e) => {
                          // mousedown 而非 click：textarea 保持焦点（combobox 模式）
                          e.preventDefault();
                          setSlashIndex(i);
                          if (c.args) {
                            setInput("/" + c.name + " ");
                            setSlashDismissed(true);
                          } else {
                            void send();
                          }
                        }}
                        onMouseMove={i === slashActive ? undefined : () => setSlashIndex(i)}
                      >
                        <span className="cc-slash-ico" aria-hidden>
                          <Icon d={c.icon} size={14} />
                        </span>
                        <span className="cc-slash-label">{c.label}</span>
                        {/* 本地化标签旁的命令名别名（与标签仅大小写不同时才渲染） */}
                        {c.label.toLowerCase() !== c.name.toLowerCase() && (
                          <span className="cc-slash-name">{c.name}</span>
                        )}
                        <span className="cc-slash-desc">{c.desc}</span>
                      </button>
                    </Fragment>
                  );
                })}
              </div>
            </div>
          )}
          {attachments.length > 0 && (
            <div className="cc-attach-list">
              {attachments.map((a, i) => (
                <div className="cc-attach-chip" key={`${a.name}-${i}`} title={`${a.name} · ${a.mimeType}`}>
                  <img className="cc-attach-thumb" src={a.dataUrl} alt={a.name || "图片"} />
                  <span className="cc-attach-name">{a.name || "图片"}</span>
                  <button
                    className="cc-attach-x"
                    onClick={() => setAttachments((prev) => prev.filter((_, j) => j !== i))}
                    aria-label={`移除 ${a.name || "图片"}`}
                    title="移除"
                  >
                    <Icon d={ICON.close} size={12} />
                  </button>
                </div>
              ))}
            </div>
          )}
          <textarea
            className="cc-input"
            ref={inputRef}
            value={input}
            onChange={(e) => {
              setInput(e.target.value);
              setSlashIndex(0);
              setSlashDismissed(false);
            }}
            onKeyDown={onKeyDown}
            onPaste={(e) => {
              // 粘贴截图 / 复制的图片文件
              const files = Array.from(e.clipboardData?.files ?? []);
              if (files.length > 0) {
                e.preventDefault();
                void addFiles(files);
              }
            }}
            placeholder="输入消息，/ 呼出命令"
            rows={1}
          />
          <div className="cc-bar">
            <div className="cc-bar-left">
              <input
                ref={fileRef}
                type="file"
                accept="image/*"
                multiple
                hidden
                onChange={(e) => {
                  const files = e.target.files;
                  if (files && files.length > 0) void addFiles(files);
                  e.target.value = ""; // 允许重复选同一文件
                }}
              />
              <button
                className="cc-attach-btn"
                onClick={() => fileRef.current?.click()}
                title="添加图片附件"
                aria-label="添加图片附件"
              >
                <Icon d={ICON.attach} size={14} />
              </button>
              <WorkspaceChip
                current={info?.workingDir || info?.root || draftCwd}
                workspaces={workspaces}
                onPick={(dir) => void newSession(dir)}
                onBrowse={() => setWsPicker(true)}
              />
              <ApprovalChip
                value={(info?.approvalPreset ?? "workspace-write") as ApprovalPreset}
                disabled={!sessionId}
                onChange={(preset) => void onApprovalChange(preset)}
              />
              {plan && <PlanChip plan={plan} onExit={() => void onExitPlan()} />}
              <ModeChip value={mode} onChange={setMode} />
            </div>
            <div className="cc-bar-right">
              <ModelChip
                value={info?.model ?? draftModel}
                catalog={catalog}
                disabled={catalog.length === 0}
                onChange={(model) => void onModelChange(model)}
              />
              {/* 生成中：同一按钮切成「停止」（方形图标），点击中断当前轮次 */}
              <button
                className={"cc-send" + (busy ? " is-stop" : "")}
                onClick={() => (busy ? void stopTurn() : void send())}
                disabled={!busy && !canSend}
                title={busy ? "停止生成" : "发送"}
                aria-label={busy ? "停止生成" : "发送"}
              >
                <svg
                  width="16"
                  height="16"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  aria-hidden="true"
                >
                  {busy ? (
                    <rect x="7" y="7" width="10" height="10" rx="1.5" fill="currentColor" stroke="none" />
                  ) : (
                    <path d="M12 19V5 M5 12l7-7 7 7" />
                  )}
                </svg>
              </button>
            </div>
          </div>
        </div>
        </div>
      )}

      {wsPicker && (
        <WorkspacePicker
          agent={agent}
          startDir={info?.workingDir || info?.root || draftCwd}
          recent={workspaces}
          native={pickNative}
          onCancel={() => setWsPicker(false)}
          onPick={(dir) => {
            setWsPicker(false);
            void newSession(dir);
          }}
        />
      )}
    </div>
  );
}
