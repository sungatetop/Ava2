// 对话条目与待回填卡片的渲染组件。
// 形态：用户消息为右对齐气泡；助手消息无气泡——轮次过程（思考 + 工具步骤，默认折叠）
// 在上、markdown 正文在下、操作行 hover 显示复制；审批/计划/问询为卡片。
import { Fragment, useState, type ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { AnswerInput } from "@ava-ai/app-sdk";
import type { ChatItem, CommandItem, CompactionItem, PendingCard, ToolStep, TurnItem, TurnUsage, UserAttachment } from "./chat-types";
import { openInCanvas } from "./canvas-bridge";
import { firstLine, formatCacheHitPercent, formatExactTokens, formatTokens, turnUsageOf, truncate, usageTotal } from "./format";
import { Popover } from "./popover";

const ICON = {
  // 用向下 chevron 表示收起，展开时旋转 180°。
  chevron: "M6 9l6 6 6-6",
  copy: "M9 9h10v11H9z M5 15V5a1 1 0 0 1 1-1h9",
  file: "M6 3h8l4 4v14H6z M14 3v4h4",
  // 用数据库图标（IconDatabaseOutlineRegular）标记 token 用量。
  db: "M12 2C7.58 2 4 3.34 4 5s3.58 3 8 3 8-1.34 8-3-3.58-3-8-3z M4 5v14c0 1.66 3.58 3 8 3s8-1.34 8-3V5 M4 12c0 1.66 3.58 3 8 3s8-1.34 8-3",
  // 命令/压缩行用 IconApiOutlineRegular（方框内 `>_` 终端提示符）。
  terminal: "M4 5h16v14H4z M8 10l2 2-2 2 M13 14h3",
} as const;

/** 工具中文名映射：读取 / 运行命令 / 写入 … */
const TOOL_LABELS: Record<string, string> = {
  read: "读取",
  read_image: "读取图片",
  write: "写入",
  edit: "编辑",
  bash: "运行命令",
  glob: "查找文件",
  grep: "搜索文件内容",
  web_search: "网页搜索",
  web_fetch: "网页获取",
  skill: "加载技能",
  task: "子任务",
  present: "展示文件",
  todo_write: "更新任务清单",
};

function toolLabel(name: string): string {
  return TOOL_LABELS[name.toLowerCase()] ?? name;
}

/** present 工具结果 `{turn, files:[{path, description}]}` 中的交付物 */
interface PresentedFile {
  path: string;
  description?: string;
}

function parsePresented(text: string | undefined): PresentedFile[] {
  if (!text) return [];
  try {
    const parsed = JSON.parse(text) as { files?: unknown };
    const files = Array.isArray(parsed.files) ? parsed.files : [];
    return files.flatMap<PresentedFile>((raw) => {
      const file = (typeof raw === "object" && raw !== null ? raw : {}) as Record<string, unknown>;
      if (typeof file.path !== "string" || !file.path) return [];
      return [
        {
          path: file.path,
          description: typeof file.description === "string" ? file.description : undefined,
        },
      ];
    });
  } catch {
    return [];
  }
}

/** 路径末段（卡片标题） */
function baseName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts.length > 0 ? parts[parts.length - 1] : path;
}

export function Icon({ d, size = 14 }: { d: string; size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d={d} />
    </svg>
  );
}

/** 复制到剪贴板（权限被拒时静默，UI 不做错误提示） */
function useCopy(): [boolean, (text: string) => void] {
  const [copied, setCopied] = useState(false);
  const copy = (text: string) => {
    navigator.clipboard
      ?.writeText(text)
      .then(() => {
        setCopied(true);
        window.setTimeout(() => setCopied(false), 1500);
      })
      .catch(() => undefined);
  };
  return [copied, copy];
}

/** 条目锚点 id（轨迹视图 → 消息流定位） */
export function itemAnchor(index: number): string {
  return `cc-item-${index}`;
}

// ---- markdown ----

function Markdown({ text }: { text: string }) {
  return (
    <div className="cc-md">
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={{ pre: CodeBlock }}>
        {text}
      </ReactMarkdown>
    </div>
  );
}

/** 代码块：语言条 + 复制 */
function CodeBlock({ children }: { children?: ReactNode }) {
  const child = Array.isArray(children) ? children[0] : children;
  const props = (child as { props?: { className?: string; children?: ReactNode } } | null)?.props;
  const lang = /language-([\w-]+)/.exec(props?.className ?? "")?.[1] ?? "text";
  const code = String(props?.children ?? "").replace(/\n$/, "");
  const [copied, copy] = useCopy();
  return (
    <div className="cc-code">
      <div className="cc-code-bar">
        <span>{lang}</span>
        <button className="cc-code-copy" onClick={() => copy(code)}>
          {copied ? "已复制" : "复制"}
        </button>
      </div>
      <pre>
        <code className={props?.className}>{code}</code>
      </pre>
    </div>
  );
}

// ---- 轮次过程区 ----

function StateDot({ step }: { step: ToolStep }) {
  if (step.running) return <span className="cc-state-dot running" aria-label="运行中" />;
  if (step.isError) return <span className="cc-state-dot err" aria-label="失败" />;
  return <span className="cc-state-dot ok" aria-label="完成" />;
}

/** todo_write 输入 → 进度摘要（`{done}/{total} 已完成`，
 *  有进行项时追加 `· {首个进行项}`，并行进行项余数作为 `+N` 后缀）。 */
function todoSummary(inputText: string | undefined): { text: string; extra: number } | null {
  if (!inputText) return null;
  try {
    const parsed = JSON.parse(inputText) as { todos?: unknown };
    const list = Array.isArray(parsed.todos) ? parsed.todos : [];
    const items = list.flatMap((raw) => {
      const t = (typeof raw === "object" && raw !== null ? raw : {}) as Record<string, unknown>;
      if (typeof t.content !== "string" || !t.content) return [];
      return [{ content: t.content, status: String(t.status ?? "pending") }];
    });
    if (items.length === 0) return null;
    const done = items.filter((t) => t.status === "completed").length;
    const active = items.filter((t) => t.status === "in_progress");
    const head = `${done}/${items.length} 已完成`;
    if (active.length === 0) return { text: head, extra: 0 };
    return { text: `${head} · ${firstLine(active[0].content, 48)}`, extra: active.length - 1 };
  } catch {
    return null;
  }
}

/** 工具步骤行：状态点 + 名称 + 摘要，展开看输入/结果。
 *  `present` 声明的交付物由轮次末尾统一渲染；`todo_write` 摘要显示清单进度。 */
function ToolRow({ step }: { step: ToolStep }) {
  const [open, setOpen] = useState(false);
  const isPresent = step.name.toLowerCase() === "present";
  const hasDetail = Boolean(step.inputText || (step.resultText && !isPresent));
  const todo = step.name.toLowerCase() === "todo_write" ? todoSummary(step.inputText ?? step.resultText) : null;
  // 失败时折叠摘要替换为**错误首行**并整行标红；运行中显示运行态。
  const summary = step.running
    ? "运行中…"
    : todo
      ? todo.text
      : firstLine(step.isError ? step.resultText ?? "" : step.inputText ?? "", 72);
  return (
    <div className="cc-tool">
      <button
        className={"cc-row" + (hasDetail ? "" : " cc-row-static")}
        aria-expanded={hasDetail ? open : undefined}
        onClick={() => hasDetail && setOpen((v) => !v)}
      >
        <StateDot step={step} />
        <span className="cc-tool-name">{toolLabel(step.name)}</span>
        {summary && (
          <>
            <span className="cc-tool-sep" aria-hidden="true" />
            <span className={"cc-row-summary" + (step.isError ? " err" : "")}>{summary}</span>
            {todo && todo.extra > 0 && <span className="cc-tool-extra">+{todo.extra}</span>}
          </>
        )}
      </button>
      {open && hasDetail && (
        <div className="cc-tool-details">
          {step.inputText && (
            <div className="cc-tool-detail-row">
              <span className="cc-tool-detail-label">输入</span>
              <pre>{step.inputText}</pre>
            </div>
          )}
          {step.resultText && !isPresent && (
            <div className={"cc-tool-detail-row" + (step.isError ? " err" : "")}>
              <span className="cc-tool-detail-label">结果</span>
              <pre>{step.resultText}</pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

/** 轮次内 `present` 声明的交付物（按 path 去重，保留最后一次声明）。 */
function presentedOf(item: TurnItem): PresentedFile[] {
  const byPath = new Map<string, PresentedFile>();
  for (const step of item.steps) {
    if (step.name.toLowerCase() !== "present" || step.isError) continue;
    for (const file of parsePresented(step.resultText)) byPath.set(file.path, file);
  }
  return [...byPath.values()];
}

/** 交付物卡片组（轮次末尾）：文件名 + 描述，可「在画布打开」。 */
function Deliverables({ files, sessionId }: { files: PresentedFile[]; sessionId?: string }) {
  return (
    <div className="cc-deliverables">
      {files.map((file) => (
        <div className="cc-deliverable" key={file.path}>
          <span className="cc-deliverable-icon">
            <Icon d={ICON.file} />
          </span>
          <span className="cc-deliverable-body">
            <span className="cc-deliverable-name" title={file.path}>
              {baseName(file.path)}
            </span>
            {file.description && <span className="cc-deliverable-desc">{file.description}</span>}
          </span>
          <button
            className="cc-deliverable-open"
            onClick={() => openInCanvas(file.path, sessionId)}
            title={file.path}
          >
            在画布打开
          </button>
        </div>
      ))}
    </div>
  );
}

/** 首行（dsh ReasoningRow 同款：不截断，溢出交给 CSS ellipsis） */
function firstLineOf(text: string): string {
  const newline = text.indexOf("\n");
  return newline === -1 ? text : text.slice(0, newline);
}

/** 流式预览摘要：最新一个「已完成段落」的首行（dsh ReasoningRow 同款，
 *  摘要随段落完成推进，而不是逐字符滚动）。 */
function latestParagraphFirstLine(text: string): string {
  let summary = "";
  let paragraphStart = 0;
  const separator = /\r?\n(?:[\t ]*\r?\n)+/g;
  while (true) {
    const nextParagraph = separator.exec(text);
    const paragraphEnd =
      nextParagraph === null ? text.length : nextParagraph.index + nextParagraph[0].indexOf("\n");
    const newline = text.indexOf("\n", paragraphStart);
    if (newline !== -1 && newline <= paragraphEnd) {
      const candidate = text.slice(paragraphStart, newline).trim();
      if (candidate !== "") summary = candidate;
    }
    if (nextParagraph === null) return summary;
    paragraphStart = nextParagraph.index + nextParagraph[0].length;
  }
}

/** 思考行（dsh web ui 对齐）：每段思考一个独立折叠行——
 *  收起时摘要取首行（流式中取最新完成段落首行）并剥离 ** 强调标记，
 *  流式摘要带 shimmer 微光；展开渲染完整 markdown（次级排版）。 */
function ReasoningRow({ thinking, live }: { thinking: string; live: boolean }) {
  const [open, setOpen] = useState(false);
  const summary = (live && !open ? latestParagraphFirstLine(thinking) : firstLineOf(thinking)).replaceAll(
    "**",
    "",
  );
  return (
    <>
      <button className="cc-row" aria-expanded={open} onClick={() => setOpen((v) => !v)}>
        <span className={"cc-chevron" + (open ? " open" : "")}>
          <Icon d={ICON.chevron} />
        </span>
        <span className="cc-tool-name">{live ? "思考中" : "思考"}</span>
        {!open && (
          <span className={"cc-row-summary" + (live ? " cc-shimmer" : "")}>{summary}</span>
        )}
      </button>
      {open && (
        <div className="cc-thinking">
          <Markdown text={thinking} />
        </div>
      )}
    </>
  );
}

/** 轮次过程区：33px 折叠头（已完成 · N 步 · reason / 处理中；token 用量在轮次末尾）。
 *  过程体按时间序交错渲染（dsh web ui 对齐）：思考段 i → 工具步骤 i → … → 末段思考。 */
function ProcessBlock({ item }: { item: TurnItem }) {
  const [open, setOpen] = useState(false);
  const live = item.streaming;
  const steps = item.steps.length;
  const reason = item.report?.reason ?? "";
  const meta = live
    ? "处理中…"
    : [
        reason === "cancelled" ? "已取消" : "已完成",
        steps > 0 ? `${steps} 步` : "",
        reason === "cancelled" ? "" : reason,
      ]
        .filter(Boolean)
        .join(" · ");
  const hasThinking = item.thinkingSegs.some((s) => s.trim() !== "");
  const empty = !hasThinking && steps === 0;
  const tailSeg = item.thinkingSegs[steps] ?? "";
  return (
    <div>
      <button
        className="cc-process-head"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        disabled={empty}
      >
        <span className={"cc-chevron" + (open ? " open" : "")}>
          <Icon d={ICON.chevron} />
        </span>
        <span className="cc-process-meta">{meta}</span>
      </button>
      {open && (
        <div className="cc-process-body">
          {item.steps.map((s, i) => {
            const seg = item.thinkingSegs[i] ?? "";
            return (
              <Fragment key={s.id || `${i}-${s.name}`}>
                {seg.trim() !== "" && <ReasoningRow thinking={seg} live={false} />}
                <ToolRow step={s} />
              </Fragment>
            );
          })}
          {tailSeg.trim() !== "" && (
            <ReasoningRow thinking={tailSeg} live={live && !item.answer} />
          )}
        </div>
      )}
    </div>
  );
}

/** 轮末 token 用量 pill：数据库图标 + 「用量 N tok」，点击弹出「本轮用量」明细 */
function UsagePill({ usage }: { usage: TurnUsage }) {
  const [open, setOpen] = useState(false);
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const total = usageTotal(usage);
  // 缓存读写已含在输入口径内：未缓存输入 = 输入 - 缓存读 - 缓存写
  const uncached = Math.max(0, usage.inputTokens - usage.cacheReadTokens - usage.cacheWriteTokens);
  const cacheHit = formatCacheHitPercent(usage.cacheReadTokens, usage.inputTokens);
  return (
    <>
      <button
        ref={setAnchor}
        className={"cc-turn-usage" + (open ? " open" : "")}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
      >
        <Icon d={ICON.db} size={15} />
        <span className="cc-turn-usage-label">用量 {formatTokens(total)} tok</span>
      </button>
      <Popover anchor={anchor} open={open} onClose={() => setOpen(false)} side="top" width={300}>
        <div className="cc-usage-pop">
          <div className="cc-usage-pop-title">
            <span className="cc-usage-pop-name">
              <Icon d={ICON.db} size={14} />
              本轮用量
            </span>
            <span className="cc-usage-pop-total">{formatExactTokens(total)}</span>
          </div>
          <div className="cc-usage-pop-rule" aria-hidden />
          <dl className="cc-usage-pop-details">
            {cacheHit !== null && (
              <>
                <dt>缓存命中</dt>
                <dd>{cacheHit}%</dd>
              </>
            )}
            <dt>未缓存输入</dt>
            <dd>{formatExactTokens(uncached)}</dd>
            {usage.cacheReadTokens > 0 && (
              <>
                <dt>缓存读取</dt>
                <dd>{formatExactTokens(usage.cacheReadTokens)}</dd>
              </>
            )}
            {usage.cacheWriteTokens > 0 && (
              <>
                <dt>缓存写入</dt>
                <dd>{formatExactTokens(usage.cacheWriteTokens)}</dd>
              </>
            )}
            <dt>输出</dt>
            <dd>{formatExactTokens(usage.outputTokens)}</dd>
          </dl>
        </div>
      </Popover>
    </>
  );
}

/** 助手轮次块：过程区 + markdown 正文 + 交付物卡片 + 末尾操作行（复制 + token 用量） */
export function AssistantTurn({
  item,
  id,
  sessionId,
}: {
  item: TurnItem;
  id?: string;
  sessionId?: string;
}) {
  const [copied, copy] = useCopy();
  const usage = turnUsageOf(item);
  const tokens = usageTotal(usage);
  const hasProcess =
    item.thinkingSegs.some((s) => s.trim() !== "") || item.steps.length > 0 || tokens > 0;
  const deliverables = presentedOf(item);
  return (
    <div className="cc-assistant" id={id}>
      {item.retryText && <div className="cc-note">↻ {truncate(item.retryText, 160)}</div>}
      {hasProcess && <ProcessBlock item={item} />}
      {item.answer && <Markdown text={item.answer} />}
      {deliverables.length > 0 && <Deliverables files={deliverables} sessionId={sessionId} />}
      {item.streaming && item.answer && <span className="cc-caret" />}
      {item.streaming && !item.answer && !hasProcess && <div className="cc-note">…</div>}
      {!item.streaming && (item.answer || (usage && tokens > 0)) && (
        <div className="cc-tail" data-actions-reveal="hover">
          <div className="cc-actions">
            {item.answer && (
              <button className="cc-icon-btn" title={copied ? "已复制" : "复制"} onClick={() => copy(item.answer)}>
                <Icon d={ICON.copy} size={15} />
              </button>
            )}
          </div>
          {usage && tokens > 0 && <UsagePill usage={usage} />}
        </div>
      )}
    </div>
  );
}

export function UserBubble({
  text,
  attachments,
  optimistic,
  id,
}: {
  text: string;
  attachments?: UserAttachment[];
  optimistic?: boolean;
  id?: string;
}) {
  return (
    <div className={"cc-user" + (optimistic ? " is-optimistic" : "")} id={id}>
      {attachments && attachments.length > 0 && (
        <div className="cc-user-images">
          {attachments.map((a, i) => (
            <img
              className="cc-user-image"
              key={i}
              src={a.dataUrl}
              alt={a.name || "图片"}
              title={a.name || a.mimeType}
            />
          ))}
        </div>
      )}
      {text && <div className="cc-user-text">{text}</div>}
    </div>
  );
}

/** 斜杠命令本地回执行（不进轮次块；error 态转红） */
export function NoticeView({ text, tone }: { text: string; tone: "info" | "error" }) {
  return <div className={"cc-notice" + (tone === "error" ? " is-error" : "")}>{text}</div>;
}

/** 命令回显气泡：右对齐，`/name` 渲染为命令 chip，
 *  参数保持普通文本。 */
function CommandEcho({ name, args }: { name: string; args: string }) {
  return (
    <div className="cc-user cc-cmd-echo">
      <div className="cc-user-text">
        <span className="cc-cmd-chip">/{name}</span>
        {args}
      </div>
    </div>
  );
}

/** 命令结果行：`>_` 图标 + 命令名 + 结果摘要；
 *  多行结果可展开。 */
function CommandResultRow({ name, outcome }: { name: string; outcome: { kind: "success" | "error"; text: string } | null }) {
  const [open, setOpen] = useState(false);
  const running = outcome === null;
  const text = outcome?.text ?? "";
  const failed = outcome?.kind === "error";
  const summary = running ? "执行中…" : text === "" ? (failed ? "指令失败" : "已完成") : firstLine(text, 90);
  const expandable = !running && text.includes("\n");
  return (
    <div className="cc-cmd-result" data-state={running ? "running" : failed ? "error" : "ok"}>
      <button
        className={"cc-row" + (expandable ? "" : " cc-row-static")}
        aria-expanded={expandable ? open : undefined}
        onClick={() => expandable && setOpen((v) => !v)}
      >
        <span className="cc-cmd-glyph">
          <Icon d={ICON.terminal} />
        </span>
        <span className="cc-tool-name">{name || "指令"}</span>
        <span className="cc-tool-sep" aria-hidden="true" />
        <span className={"cc-row-summary" + (failed ? " err" : "")}>{summary}</span>
      </button>
      {open && expandable && (
        <div className="cc-tool-details">
          <pre className={failed ? "err" : undefined}>{text}</pre>
        </div>
      )}
    </div>
  );
}

/** 命令条目：回显气泡 + 结果行作为同一条目的两段渲染。
 *
 *  `sourceEventSeq` 有值 = 命令产出了领域事件（压缩标记 / 目标横幅 / 计划状态），
 *  该事件已由富卡片承载，故不再渲染回执结果行（handler 结果不回声）。 */
export function CommandView({ item }: { item: CommandItem }) {
  const rich = typeof item.sourceEventSeq === "number";
  return (
    <div className="cc-cmd">
      <CommandEcho name={item.name} args={item.args} />
      {!rich && <CommandResultRow name={item.name} outcome={item.outcome} />}
    </div>
  );
}

/** 上下文压缩标记：图标 + 「上下文已压缩」+ 计数摘要，
 *  有摘要时可展开查看。 */
export function CompactionView({ item }: { item: CompactionItem }) {
  const [open, setOpen] = useState(false);
  const expandable = item.summary !== null;
  const summary = item.running
    ? "正在压缩…"
    : item.error
      ? `压缩失败：${item.error}`
      : item.shadowedItemCount !== null && item.shadowedTokenCount !== null
        ? `已压缩 ${item.shadowedItemCount} 条历史记录（约 ${formatTokens(item.shadowedTokenCount)} tokens）`
        : expandable
          ? "点击查看压缩摘要"
          : "压缩摘要不可用";
  return (
    <div className="cc-compaction">
      <button
        className="cc-compaction-btn"
        disabled={!expandable}
        aria-expanded={expandable ? open : undefined}
        onClick={() => setOpen((v) => !v)}
      >
        <span className="cc-compaction-leading" aria-hidden="true">
          <span className="cc-compaction-icon">
            <Icon d={ICON.terminal} />
          </span>
          <span className={"cc-chevron" + (open ? " open" : "")}>
            <Icon d={ICON.chevron} />
          </span>
        </span>
        <span className="cc-compaction-title">上下文已压缩</span>
        <span className="cc-tool-sep" aria-hidden="true" />
        <span className={"cc-compaction-summary" + (item.error ? " err" : "")}>{summary}</span>
      </button>
      {open && item.summary !== null && (
        <div className="cc-compaction-body">
          <Markdown text={item.summary} />
        </div>
      )}
    </div>
  );
}

/** 条目流：用户气泡 + 助手轮次块（每项带锚点 id，供轨迹视图跳转定位） */
export function ChatItems({ items, sessionId }: { items: ChatItem[]; sessionId?: string }) {
  return (
    <>
      {items.map((item, i) =>
        item.kind === "user" ? (
          <UserBubble
            key={i}
            id={itemAnchor(i)}
            text={item.text}
            attachments={item.attachments}
            optimistic={item.optimistic}
          />
        ) : item.kind === "notice" ? (
          <NoticeView key={i} text={item.text} tone={item.tone} />
        ) : item.kind === "command" ? (
          <CommandView key={i} item={item} />
        ) : item.kind === "compaction" ? (
          <CompactionView key={i} item={item} />
        ) : (
          <AssistantTurn key={i} id={itemAnchor(i)} item={item} sessionId={sessionId} />
        ),
      )}
    </>
  );
}

// ---- 待回填卡片 ----

function Button({
  label,
  onClick,
  tone = "plain",
}: {
  label: string;
  onClick: () => void;
  tone?: "primary" | "plain" | "danger";
}) {
  return (
    <button
      className={
        "cc-btn" + (tone === "primary" ? " is-primary" : tone === "danger" ? " is-danger" : "")
      }
      onClick={onClick}
    >
      {label}
    </button>
  );
}

function safeJson(value: unknown): string {
  try {
    return typeof value === "string" ? value : JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

/** 会话级授权不可覆盖的破坏性命令首词（与后端 `onion::approval::session_grant` 同口径）。
 *  命中时卡片不提供「本会话允许」，破坏性命令必须逐次审批。 */
const SESSION_GRANT_DENIED = [
  "rm", "del", "erase", "rmdir", "rd", "format", "mkfs", "dd", "shutdown", "reboot", "sudo",
  "doas", "diskpart", "reg", "stop-process", "remove-item", "kill",
];

/** 「本会话允许」的授权粒度标签（命令首词）；不可授权时返回 null（不展示第三按钮）。 */
function sessionGrantLabel(input: unknown): string | null {
  if (!input || typeof input !== "object") return null;
  const command = (input as { command?: unknown }).command;
  if (typeof command !== "string") return null;
  const normalized = command.trim().toLowerCase();
  if (!normalized) return null;
  // 组合命令（链式 / 管道 / 重定向 / 命令替换）不提供会话级授权：一次授权会扩散到
  // 同族任意命令，与「逐次审批」的安全承诺冲突。
  if (["&&", "||", ";", "|", ">", "<", "`", "$("].some((m) => normalized.includes(m))) return null;
  const first = normalized.split(/\s+/)[0];
  return SESSION_GRANT_DENIED.includes(first) ? null : first;
}

/** 工具审批卡：warn 顶条 + 操作右对齐 */
export function ApprovalCardView({
  card,
  onDecision,
}: {
  card: PendingCard;
  onDecision: (requestId: string, decision: "approve" | "approve_for_session" | "deny") => void;
}) {
  const sandboxRetry = card.reason === "sandbox-retry";
  // 沙箱已证实越界的命令不做会话级批量授权：一次授权等于同族命令永久免首次审批，
  // 与「越界执行逐次申请」的承诺冲突（对齐破坏性命令的处理）。
  const grantLabel = sandboxRetry ? null : sessionGrantLabel(card.input);
  return (
    <div className="cc-card is-approval">
      <div className="cc-card-strip">
        {sandboxRetry ? "沙箱拒绝后申请越界执行" : "需要你的确认"}
      </div>
      <div className="cc-card-body">
        <div className="cc-card-title">工具审批：{card.tool}</div>
        <div className="cc-card-meta">{truncate(safeJson(card.input), 600)}</div>
      </div>
      <div className="cc-card-actions">
        <Button label="拒绝" tone="danger" onClick={() => onDecision(card.requestId, "deny")} />
        <Button label="批准" tone="primary" onClick={() => onDecision(card.requestId, "approve")} />
        {grantLabel ? (
          <Button
            label={`本会话允许 ${card.tool}: ${grantLabel}`}
            onClick={() => onDecision(card.requestId, "approve_for_session")}
          />
        ) : null}
      </div>
    </div>
  );
}

/** 计划评审卡：warn 顶条 + 底部分列按钮 */
export function PlanReviewCardView({
  card,
  onVerdict,
}: {
  card: PendingCard;
  onVerdict: (requestId: string, verdict: "approve" | "keep_planning" | "dismissed") => void;
}) {
  return (
    <div className="cc-card">
      <div className="cc-card-strip">计划待确认</div>
      <div className="cc-card-body">
        <Markdown text={card.plan ?? ""} />
      </div>
      <div className="cc-card-actions">
        <Button label="放弃" onClick={() => onVerdict(card.requestId, "dismissed")} />
        <Button label="继续规划" onClick={() => onVerdict(card.requestId, "keep_planning")} />
        <Button label="批准执行" tone="primary" onClick={() => onVerdict(card.requestId, "approve")} />
      </div>
    </div>
  );
}

interface QuestionView {
  id: string;
  question: string;
  header?: string;
  options?: Array<{ label: string; description?: string }>;
  multiSelect?: boolean;
}

/** 人工问询卡：选项行 40px，选中高亮 */
export function QuestionCardView({
  card,
  onSubmit,
}: {
  card: PendingCard;
  onSubmit: (requestId: string, answers: AnswerInput[]) => void;
}) {
  const questions = Array.isArray(card.questions) ? (card.questions as QuestionView[]) : [];
  const [selected, setSelected] = useState<Record<string, string[]>>({});
  const [custom, setCustom] = useState<Record<string, string>>({});
  const toggle = (qid: string, label: string, multi: boolean) => {
    setSelected((prev) => {
      const current = prev[qid] ?? [];
      if (!multi) return { ...prev, [qid]: current.includes(label) ? [] : [label] };
      return {
        ...prev,
        [qid]: current.includes(label) ? current.filter((l) => l !== label) : [...current, label],
      };
    });
  };
  const submit = () => {
    const answers: AnswerInput[] = questions.map((q) => ({
      id: q.id,
      selected: selected[q.id] ?? [],
      custom: custom[q.id]?.trim() || undefined,
    }));
    onSubmit(card.requestId, answers);
  };
  return (
    <div className="cc-card is-question">
      {questions.map((q) => (
        <div className="cc-question" key={q.id}>
          {q.header && <div className="cc-question-head">{q.header}</div>}
          <div className="cc-question-text">{q.question}</div>
          {(q.options ?? []).map((opt, idx) => {
            const on = (selected[q.id] ?? []).includes(opt.label);
            return (
              <button
                key={opt.label}
                className={"cc-option" + (on ? " on" : "")}
                onClick={() => toggle(q.id, opt.label, q.multiSelect === true)}
              >
                <span className="cc-option-mark">{on ? "✓" : idx + 1}</span>
                <span>
                  {opt.label}
                  {opt.description && <span className="cc-option-desc"> — {opt.description}</span>}
                </span>
              </button>
            );
          })}
          <input
            className="cc-question-custom"
            placeholder="自定义回答（可选）"
            value={custom[q.id] ?? ""}
            onChange={(e) => setCustom((prev) => ({ ...prev, [q.id]: e.target.value }))}
          />
        </div>
      ))}
      <div className="cc-card-actions" style={{ padding: "var(--ava-space-2, 8px) 0 0" }}>
        <Button label="提交" tone="primary" onClick={submit} />
      </div>
    </div>
  );
}
