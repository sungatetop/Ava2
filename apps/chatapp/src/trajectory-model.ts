// 轨迹视图数据模型：ChatItem[] → 有序记录（纯函数派生，无网络请求）。
// 设计参考 ava_square TrajectoryView：泳道概览按序号等宽排布（不表达时长）、
// 回合分组记录表（tag chip + 单行摘要 + 右对齐指标 + 可展开详情）。
import type { ChatItem, TurnItem } from "./chat-types";
import { firstLine, turnUsageOf } from "./format";

/** 记录类别：泳道与色标共用（kind 语义对齐 ava_square trajectory.js） */
export type TrajKind = "user" | "message" | "thinking" | "tool" | "system" | "context" | "error";

/** 记录行左侧短标签（对齐 ava_square：U / MSG / THINK / TOOL / SYS / CTX / ERR） */
const TAG_BY_KIND: Record<TrajKind, string> = {
  user: "U",
  message: "MSG",
  thinking: "THINK",
  tool: "TOOL",
  system: "SYS",
  context: "CTX",
  error: "ERR",
};

/** 单条记录（表格行 / 泳道块共用同一对象） */
export interface TrajCell {
  /** 稳定 id（c1、c2…），选中定位键 */
  id: string;
  /** 全局序号（1 起；泳道等宽定位依据） */
  index: number;
  /** 回合序号（视图内出现顺序，1 起；0 = 无轮次的散记录） */
  turn: number;
  kind: TrajKind;
  tag: string;
  /** 主摘要（工具名为等宽名称，其余为正文单行摘要） */
  title: string;
  /** 次级摘要（表格行灰色补充列；主摘要即全文时省略） */
  summary?: string;
  /** 展开详情（pre 正文；工具为 input/output，其余为全文） */
  detail?: string;
  /** 消息行 token 数 */
  tokens?: number;
  /** 工具执行结果：true 成功 / false 失败 / undefined 进行中或非工具 */
  ok?: boolean;
  running?: boolean;
  /** 搜索文本（tag + title + summary + detail 小写拼接） */
  searchText: string;
}

/** 回合分组（表格粘性轮头） */
export interface TrajTurn {
  /** 视图内回合序号（1 起；0 = 会话前置散记录） */
  turn: number;
  /** 收束原因（turn_report.reason，cancelled 等） */
  reason: string;
  /** 步数（turn_report.steps；无报告时回落为工具步数） */
  steps: number;
  streaming: boolean;
  cells: TrajCell[];
  tools: number;
  input: number;
  output: number;
  failed: number;
}

export interface TrajStats {
  turns: number;
  cells: number;
  tools: number;
  input: number;
  output: number;
  failed: number;
}

export interface TrajModel {
  cells: TrajCell[];
  turns: TrajTurn[];
  stats: TrajStats;
}

/** 单行摘要（折叠空白 + 截断） */
const oneLine = (text: string, max = 160): string => firstLine(text, max);

/** 主/次摘要：次级摘要与主摘要相同时省略（避免表格行重复展示同一段文字） */
function splitSummary(text: string, shortMax: number, longMax = 160): { title: string; summary?: string } {
  const title = oneLine(text, shortMax);
  const summary = firstLine(text, longMax);
  return { title, summary: summary !== title ? summary : undefined };
}

/** 工具输入/输出详情正文 */
function toolDetail(inputText: string, resultText: string | undefined): string {
  return [
    inputText.trim() ? `input:\n${inputText.trim()}` : "",
    resultText?.trim() ? `output:\n${resultText.trim()}` : "",
  ]
    .filter(Boolean)
    .join("\n\n");
}

/**
 * 把非 turn 条目归入回合：用户消息永远归属「下一个」回合（先于 turn 到达），
 * notice / command / compaction 归属「当前」回合（ turn 的产物），会话开头无轮次时
 * 一并归属下一个回合，仍无着落的归入序号 0 的散记录组。
 */
function assignTurns(items: ChatItem[]): number[] {
  const turnOrdinal = new Map<number, number>(); // item index → 回合序号
  let counter = 0;
  for (let i = 0; i < items.length; i++) {
    if (items[i].kind === "turn") turnOrdinal.set(i, ++counter);
  }
  const nextTurnOrdinal = (from: number): number => {
    for (const [idx, ord] of turnOrdinal) if (idx > from) return ord;
    return 0;
  };
  const ordinals = new Array<number>(items.length).fill(0);
  let current = 0;
  for (let i = 0; i < items.length; i++) {
    const known = turnOrdinal.get(i);
    if (known !== undefined) {
      current = known;
      ordinals[i] = known;
      continue;
    }
    const item = items[i];
    ordinals[i] =
      item.kind === "user"
        ? nextTurnOrdinal(i) || counter + 1 // 无后续回合（进行中的乐观气泡）→ 预占下一序号
        : current || nextTurnOrdinal(i);
  }
  return ordinals;
}

/** 单个回合条目 → 记录行（思考 / 工具步骤 / 回答 / 重试错误）。
 *  思考段与工具步骤按时间序交错（dsh web ui 对齐）：segs[i] 先于 steps[i]，
 *  末段思考先于回答。id / index 由 buildTrajectory 末尾统一重编号。 */
function cellsFromTurn(item: TurnItem, turn: number): TrajCell[] {
  const cells: TrajCell[] = [];
  const usage = turnUsageOf(item);
  const finalize = (
    cell: Omit<TrajCell, "searchText" | "id" | "index"> & { id?: string; index?: number },
  ): TrajCell => {
    const full: TrajCell = {
      id: "",
      index: 0,
      ...cell,
      searchText: [cell.tag, cell.title, cell.summary ?? "", cell.detail ?? ""].join("\n").toLowerCase(),
    } as TrajCell;
    return full;
  };
  let firstThinking = true;
  const thinkingCell = (seg: string) => {
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
        tokens: usage && firstThinking ? usage.outputTokens : undefined,
      }),
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
        summary: step.inputText.trim() ? firstLine(step.inputText, 100) : undefined,
        detail: toolDetail(step.inputText, step.resultText),
        ok: step.running ? undefined : !step.isError,
        running: step.running,
      }),
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
        tokens: usage?.outputTokens,
      }),
    );
  }
  if (item.retryText) {
    cells.push(
      finalize({
        turn,
        kind: "error",
        tag: TAG_BY_KIND.error,
        title: item.streaming ? "请求重试中" : "请求失败重试",
        summary: firstLine(item.retryText, 100),
        detail: item.retryText,
      }),
    );
  }
  return cells;
}

/** 附件数摘要（用户消息行次级文本） */
function attachmentNote(count: number): string {
  return `[图片 ×${count}]`;
}

/**
 * ChatItem[] → 轨迹模型。回合序号按视图内出现顺序计
 *（durable turn 每轮从 1 重新计数，不适合直接展示）。
 */
export function buildTrajectory(items: ChatItem[]): TrajModel {
  const ordinals = assignTurns(items);
  const cells: TrajCell[] = [];
  const turnMap = new Map<number, TrajTurn>();
  const turns: TrajTurn[] = [];
  const ensureTurn = (turn: number): TrajTurn => {
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
        failed: 0,
      };
      turnMap.set(turn, group);
      turns.push(group);
    }
    return group;
  };

  items.forEach((item, i) => {
    const turn = ordinals[i];
    const finalize = (
      cell: Omit<TrajCell, "searchText" | "id" | "index"> & { id?: string; index?: number },
    ): TrajCell =>
      ({
        id: "",
        index: 0,
        ...cell,
        searchText: [cell.tag, cell.title, cell.summary ?? "", cell.detail ?? ""].join("\n").toLowerCase(),
      }) as TrajCell;
    switch (item.kind) {
      case "user": {
        const text = item.text.trim();
        const note = item.attachments?.length ? attachmentNote(item.attachments.length) : "";
        const { title, summary } = splitSummary(text || note, 80);
        cells.push(
          finalize({
            turn,
            kind: "user",
            tag: TAG_BY_KIND.user,
            title,
            summary,
            detail: text || note,
          }),
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
            detail: item.text,
          }),
        );
        break;
      case "command": {
        const outcome =
          item.outcome === null ? "执行中" : item.outcome.kind === "success" ? "完成" : "失败";
        cells.push(
          finalize({
            turn,
            kind: "context",
            tag: TAG_BY_KIND.context,
            title: `/${item.name}${item.args}`,
            summary: `${outcome} · ${firstLine(item.outcome?.text ?? "", 60)}`,
            detail: item.outcome?.text ?? "",
          }),
        );
        break;
      }
      case "compaction": {
        const title = item.error
          ? "压缩失败"
          : item.running
            ? "上下文压缩中"
            : `压缩 · ${item.shadowedItemCount ?? "?"} 条`;
        cells.push(
          finalize({
            turn,
            kind: "context",
            tag: TAG_BY_KIND.context,
            title,
            summary: item.error ?? (item.summary ? firstLine(item.summary, 100) : undefined),
            detail: item.summary ?? item.error ?? "",
          }),
        );
        break;
      }
    }
  });

  // 统一重编号：全局序号（泳道等宽定位）与稳定 id（选中/React key）
  cells.forEach((cell, i) => {
    cell.index = i + 1;
    cell.id = `c${i + 1}`;
  });

  // 记录落组 + 回合统计（丢弃空组）
  for (const cell of cells) {
    const group = ensureTurn(cell.turn);
    group.cells.push(cell);
    if (cell.kind === "tool") group.tools += 1;
    if (cell.ok === false) group.failed += 1;
  }
  const filled = turns.filter((group) => group.cells.length > 0);
  // token 落组：逐回合取同回合 turn 条目的用量（report 优先，多 turn 归同组时求和）
  const usageByTurn = new Map<number, { input: number; output: number }>();
  let input = 0;
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
    // 无 turn_report 的回放轮次：steps 回落为工具步数
    if (group.steps === 0) group.steps = group.tools;
    input += group.input;
    output += group.output;
    tools += group.tools;
    failed += group.failed;
  }

  return {
    cells,
    turns: filled,
    stats: { turns: filled.filter((g) => g.turn > 0).length, cells: cells.length, tools, input, output, failed },
  };
}
