// 事件帧聚合：onion SessionNotice（松散 JSON）→ ChatItem[] 的纯函数 reducer。
// 帧形状契约见 crates/onion/session.rs（SessionEvent / LiveEvent / StreamChunk）：
// - durable SessionEvent：snake_case `type` 标签
// - live LiveEvent：外部标签 PascalCase（AssistantStream / Durable / ...）
// - StreamChunk：外部标签（Text/Thinking/ToolCallStart/ToolCallInput；End 为字符串）
// 本文件不持有状态，ChatView 以 setState(prev => applyFrame(prev, frame)) 消费。
import type { AgentEventFrame } from "@ava-ai/app-sdk";
import type { ChatItem, ToolStep, TurnItem, TurnReport, TurnUsage, UserAttachment } from "./chat-types";

// ---- 松散帧形状助手（集中断言，避免调用点散布 as） ----

function asRecord(value: unknown): Record<string, unknown> | undefined {
  return typeof value === "object" && value !== null ? (value as Record<string, unknown>) : undefined;
}

function asString(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function textBlocks(message: unknown): string {
  const m = asRecord(message);
  const blocks = Array.isArray(m?.blocks) ? (m.blocks as unknown[]) : [];
  return blocks
    .map((b) => (asRecord(b)?.type === "text" ? asString(asRecord(b)?.text) : ""))
    .join("");
}

/** assistant_message 思考块文本列表（onion `ContentBlock::Thinking { thinking }`，
 *  serde 标签 `type: "thinking"`；每步至多一块，防御式取数组）。 */
function thinkingBlocks(message: unknown): string[] {
  const m = asRecord(message);
  const blocks = Array.isArray(m?.blocks) ? (m.blocks as unknown[]) : [];
  return blocks
    .map((b) => (asRecord(b)?.type === "thinking" ? asString(asRecord(b)?.thinking) : ""))
    .filter((text) => text !== "");
}

/** user_message 图片块 → 附件（onion `ContentBlock::Image { image_url }`；回放时即 data URL）。
 *  服务端不持久化文件名，故回放态 name 为空串（渲染回退为「图片」）。 */
function imageBlocks(message: unknown): UserAttachment[] {
  const m = asRecord(message);
  const blocks = Array.isArray(m?.blocks) ? (m.blocks as unknown[]) : [];
  return blocks.flatMap<UserAttachment>((b) => {
    const block = asRecord(b);
    if (block?.type !== "image") return [];
    const dataUrl = asString(block.image_url);
    if (!dataUrl) return [];
    const mime = /^data:([^;,]+)/.exec(dataUrl)?.[1] ?? "image/*";
    return [{ name: "", mimeType: mime, dataUrl }];
  });
}

/** tool_result content 数组中的 text 块拼接 */
function contentText(content: unknown): string {
  const blocks = Array.isArray(content) ? (content as unknown[]) : [];
  return blocks
    .map((b) => (asRecord(b)?.type === "text" ? asString(asRecord(b)?.text) : ""))
    .join("");
}

/** user_message 的 source 非空 = 宿主注入（baseline/env/...），不渲染为对话 */
function isInjected(message: unknown): boolean {
  const m = asRecord(message);
  return m?.source != null;
}

function asNumber(value: unknown): number {
  return typeof value === "number" ? value : -1;
}

/** 非负数值（token 计数缺省 0） */
function asCount(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

/** turn_report.usage（onion `Usage` 的 snake_case 投影）→ camelCase */
function turnUsage(raw: unknown): TurnUsage | undefined {
  const u = asRecord(raw);
  if (!u) return undefined;
  return {
    inputTokens: asCount(u.input_tokens),
    outputTokens: asCount(u.output_tokens),
    cacheReadTokens: asCount(u.cache_read_tokens),
    cacheWriteTokens: asCount(u.cache_write_tokens),
  };
}

/** 两段用量相加（缺省一侧视为 0，两侧皆空返回 undefined） */
function sumUsage(base: TurnUsage | undefined, add: TurnUsage | undefined): TurnUsage | undefined {
  if (!add) return base;
  const b = base ?? { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheWriteTokens: 0 };
  return {
    inputTokens: b.inputTokens + add.inputTokens,
    outputTokens: b.outputTokens + add.outputTokens,
    cacheReadTokens: b.cacheReadTokens + add.cacheReadTokens,
    cacheWriteTokens: b.cacheWriteTokens + add.cacheWriteTokens,
  };
}

// ---- turn 条目操作 ----

/** `turn_report` 帧 → 轮次收束摘要（消息流渲染与 lastTurn 补齐共用） */
export function turnReportFromFrame(frame: AgentEventFrame): TurnReport {
  return {
    turn: asNumber(frame.turn),
    steps: asNumber(frame.steps),
    reason: asString(frame.reason),
    usage: turnUsage(frame.usage),
  };
}

function isTurn(item: ChatItem): item is TurnItem {
  return item.kind === "turn";
}

/** 打开（或复用）指定 turn 的轮次块；turn=-1 时仅复用未收束的流式块。
 *  压缩标记等非 turn 条目会把流式块挤离数组末尾——此时必须向前就地复用
 *  该块（轮次严格串行，未收束块至多一个），否则会在末尾盲目新开，留下
 *  永不收束的「处理中…」幽灵块。 */
function openTurn(items: ChatItem[], turn: number): ChatItem[] {
  const last = items[items.length - 1];
  if (last && isTurn(last) && last.streaming && (last.turn === turn || last.turn === -1)) {
    if (last.turn === -1 && turn !== -1) {
      return [...items.slice(0, -1), { ...last, turn }];
    }
    return items;
  }
  for (let i = items.length - 1; i >= 0; i--) {
    const it = items[i];
    if (isTurn(it) && it.streaming && (it.turn === turn || it.turn === -1)) {
      const patched = it.turn === -1 && turn !== -1 ? { ...it, turn } : it;
      return [...items.slice(0, i), patched, ...items.slice(i + 1)];
    }
    // 已收束且带明确轮次号的块：越过它即停（其后的流式块不属于更早的轮次）
    if (isTurn(it) && !it.streaming && it.turn !== -1 && it.turn !== turn) break;
  }
  const fresh: TurnItem = { kind: "turn", turn, thinkingSegs: [], steps: [], answer: "", streaming: true };
  return [...items, fresh];
}

/** 对最后一个轮次块原地变换；不存在时静默跳过（快照事件不会出现该场景）。 */
function withLastTurn(items: ChatItem[], patch: (t: TurnItem) => TurnItem): ChatItem[] {
  const last = items[items.length - 1];
  if (!last || !isTurn(last)) return items;
  return [...items.slice(0, -1), patch(last)];
}

/** 落定回答并入：live 流式已把同段文本累进 answer，durable echo 以
 *  「后缀命中」幂等跳过（durable 事件在步流落定后才广播，echo 到达时该段
 *  必已完整）；回放态逐步累加——中间步文本与 live 口径一致不再被覆盖。 */
function mergeAnswer(current: string, incoming: string): string {
  if (!incoming) return current;
  return current.endsWith(incoming) ? current : current + incoming;
}

/** 落定 assistant_message 的思考块并入分段：段 i 先于步骤 i、末段先于回答，
 *  维持不变量 `thinkingSegs.length ≤ steps.length + 1`。三态并入：
 *  - 末段空隙（segs.length === steps.length + 1）：替换末段——重放态末段为空
 *    等价 push；live 态为同源内容，替换即幂等；
 *  - 末段内容与块相同：live 的 durable echo 在本步 tool_call 已落后到达，
 *    思考已由 chunks 累积在同位，跳过防重复；
 *  - 其余：push 新段（重放态新步思考）。 */
function mergeThinkingSegs(turn: TurnItem, blocks: string[]): string[] {
  if (blocks.length === 0) return turn.thinkingSegs;
  let segs = turn.thinkingSegs;
  for (const block of blocks) {
    if (segs.length === turn.steps.length + 1) {
      segs = [...segs.slice(0, -1), block];
    } else if (segs[segs.length - 1] !== block) {
      segs = [...segs, block];
    }
  }
  return segs;
}

/** 对当前流式轮次块就地变换：块可能被压缩标记等条目挤离末尾，从后向前
 *  定位（轮次串行，未收束块至多一个）；无流式块时在末尾隐式新开（turn=-1）。 */
function withStreamingTurn(items: ChatItem[], patch: (t: TurnItem) => TurnItem): ChatItem[] {
  for (let i = items.length - 1; i >= 0; i--) {
    const it = items[i];
    if (isTurn(it) && it.streaming) {
      return [...items.slice(0, i), patch(it), ...items.slice(i + 1)];
    }
  }
  const fresh: TurnItem = { kind: "turn", turn: -1, thinkingSegs: [], steps: [], answer: "", streaming: true };
  return [...items, fresh];
}

/** todo_write 帧 → 归一待办清单（content/status 白名单，异常条目丢弃） */
export function todosFromEvent(raw: unknown): { content: string; status: "pending" | "in_progress" | "completed" }[] {
  const ev = asRecord(raw);
  const list = Array.isArray(ev?.todos) ? (ev.todos as unknown[]) : [];
  return list.flatMap((t) => {
    const item = asRecord(t);
    const content = asString(item?.content).trim();
    const status = item?.status;
    if (!content) return [];
    return [
      {
        content,
        status: status === "in_progress" ? "in_progress" : status === "completed" ? "completed" : "pending",
      },
    ];
  });
}

/** durable SessionEvent（todo_write 之外的变体聚合为轮次块/用户气泡） */

function applyDurable(items: ChatItem[], raw: unknown): ChatItem[] {
  const ev = asRecord(raw);
  if (!ev) return items;
  switch (ev.type) {
    case "turn_start":
      return openTurn(items, asNumber(ev.turn));

    case "user_message": {
      if (isInjected(ev.message)) return items;
      const text = textBlocks(ev.message);
      const attachments = imageBlocks(ev.message);
      // 纯附件消息（无文字）同样是有效用户输入
      if (!text && attachments.length === 0) return items;
      const settled: ChatItem =
        attachments.length > 0 ? { kind: "user", text, attachments } : { kind: "user", text };
      // 乐观气泡对账：turn_start 可能先于 user_message 到达并已打开轮次块，
      // 乐观气泡不再位于末尾，故从后向前查找同文本（且同附件数）乐观气泡就地转正。
      for (let i = items.length - 1; i >= 0; i--) {
        const it = items[i];
        if (
          it.kind === "user" &&
          it.optimistic === true &&
          it.text === text &&
          (it.attachments?.length ?? 0) === attachments.length
        ) {
          return items.map((item, j) => (j === i ? settled : item));
        }
        // 越过本轮已收束的轮次块即停止（乐观气泡必在其后）
        if (it.kind === "turn" && !it.streaming && it.turn !== -1) break;
      }
      // 快照重放无乐观气泡：turn_start 先于 user_message 落盘，本轮轮次块已
      // 打开（streaming，可能被压缩标记等条目挤离末尾）。用户气泡须插到该块
      // 之前，否则轮次块被挤离末尾，后续 assistant_message / turn_end 会给同一
      // 轮另开一个块，留下永不收束的幽灵块。
      for (let i = items.length - 1; i >= 0; i--) {
        const it = items[i];
        if (isTurn(it) && it.streaming) {
          // 插到流式块之前（slice(i) 保留该块）：写成 slice(i+1) 会把整个
          // 轮次块替换成气泡——普通场景下空块随后被 openTurn 重建而不可见，
          // 会话重放（turn_start 先于 user_message 落盘）时则吞块导致错序。
          return [...items.slice(0, i), settled, ...items.slice(i)];
        }
        if (isTurn(it) && !it.streaming && it.turn !== -1) break;
      }
      return [...items, settled];
    }

    case "tool_call": {
      const call = asRecord(ev.call);
      if (!call) return items;
      const id = asString(call.id);
      const inputText = asString(call.input);
      return withStreamingTurn(openTurn(items, asNumber(ev.turn)), (t) => {
        // live ToolCallStart 已先行占位：同 id 去重并采纳 durable 全量输入，避免双行
        if (id !== "" && t.steps.some((s) => s.id === id)) {
          return {
            ...t,
            steps: t.steps.map((s) =>
              s.id === id ? { ...s, inputText: inputText || s.inputText } : s,
            ),
          };
        }
        const step: ToolStep = {
          id,
          name: asString(call.name),
          inputText,
          running: true,
        };
        return { ...t, steps: [...t.steps, step] };
      });
    }

    case "tool_result": {
      const result = asRecord(ev.result);
      if (!result) return items;
      const callId = asString(result.tool_call_id);
      const isError = result.is_error === true;
      const resultText = contentText(result.content);
      return withStreamingTurn(items, (t) => ({
        ...t,
        steps: t.steps.map((s) =>
          s.id === callId ? { ...s, running: false, isError, resultText } : s,
        ),
      }));
    }

    case "assistant_message": {
      const text = textBlocks(ev.message);
      // 中间步 assistant_message（携带 tool_use）也会到达；只更新回答文本，
      // 不收束轮次块——收束仅由 turn_end 承担，否则后续 tool_call 会误开新块。
      const stepUsage = turnUsage(ev.usage);
      return withStreamingTurn(openTurn(items, asNumber(ev.turn)), (t) => ({
        ...t,
        // live 流式累加、durable echo 后缀幂等跳过（回放态逐步累加，中间步
        // 文本不再被末步覆盖——与 live 口径一致）
        answer: mergeAnswer(t.answer, text),
        usage: sumUsage(t.usage, stepUsage),
        // 落定消息携带本步思考块（onion final_blocks）：见 mergeThinkingSegs
        thinkingSegs: mergeThinkingSegs(t, thinkingBlocks(ev.message)),
      }));
    }

    case "assistant_attempt": {
      const error = asString(ev.error);
      if (!error) return items;
      return withStreamingTurn(items, (t) => ({ ...t, retryText: error }));
    }

    case "turn_end": {
      // 收束全部流式轮次块，而非仅末块：压缩标记 / 富卡片等非 turn 条目可能
      // 已把轮次块挤离末尾，只看末块会静默跳过，留下永不落定的「处理中…」
      // 幽灵块（历史上每次轮末上下文压缩都会产生一个）。单会话轮次严格串行，
      // turn_end 到达即当前轮次结束，残留 streaming 块一律落定。
      const anyStreaming = items.some((it) => isTurn(it) && it.streaming);
      if (!anyStreaming) return items;
      // durable turn_end 携带收束原因：重建轮次 report（live 侧由 turn_report
      // 帧承载同一份数据；两者字段同源，回放后轮头 reason 不再丢失）。
      const reason = asString(ev.reason);
      return items.map((it) => {
        if (!isTurn(it) || !it.streaming) return it;
        return {
          ...it,
          streaming: false,
          report: {
            turn: it.turn,
            steps: it.steps.length,
            reason,
            // turn_report 帧可能先行（其 usage 为权威汇总）；durable echo 补写时保留
            usage: it.usage ?? it.report?.usage,
          },
        };
      });
    }

    // ---- 斜杠命令（command_run + command_done 帧） ----
    case "command_run": {
      const id = asString(ev.command_id);
      if (!id) return items;
      const args = asString(ev.args);
      return [
        ...items,
        { kind: "command", id, name: asString(ev.name), args: args ? " " + args : "", outcome: null },
      ];
    }

    case "command_done": {
      const id = asString(ev.command_id);
      if (!id) return items;
      const kind = ev.kind === "error" ? "error" : "success";
      const text = asString(ev.text);
      // sourceEventSeq：命令产出了领域事件（压缩 / 目标 / 计划），由富卡片承载，
      // 回执文本不再单独渲染（由富卡片承载时 handler 结果不回声）。
      const seq = typeof ev.source_event_seq === "number" ? ev.source_event_seq : null;
      return items.map((item) =>
        item.kind === "command" && item.id === id
          ? { ...item, outcome: { kind, text }, sourceEventSeq: seq }
          : item,
      );
    }

    // ---- 上下文压缩（时间线留一条可展开标记） ----
    case "compaction_start": {
      // 把 `/compact` 命令与其压缩事务折叠成同一条：标记出现即取代
      // 末尾那条 compact 命令结果行，避免「已请求压缩」与标记重复。
      const last = items[items.length - 1];
      const base =
        last?.kind === "command" && last.name === "compact" ? items.slice(0, -1) : items;
      return [...base, { kind: "compaction", summary: null, shadowedItemCount: null, shadowedTokenCount: null, running: true }];
    }

    case "compaction_summary": {
      const summary = asString(ev.summary).trim();
      const count = typeof ev.shadowed_count === "number" ? ev.shadowed_count : null;
      const tokens = typeof ev.shadowed_tokens === "number" ? ev.shadowed_tokens : null;
      const marker: ChatItem = {
        kind: "compaction",
        summary: summary === "" ? null : summary,
        shadowedItemCount: count,
        shadowedTokenCount: tokens,
      };
      // 就地收束进行中的标记（压缩事务不跨轮次，末尾即当前标记）
      const last = items[items.length - 1];
      if (last?.kind === "compaction" && last.running) {
        return [...items.slice(0, -1), marker];
      }
      return [...items, marker];
    }

    case "compaction_end": {
      const error = asString(ev.error);
      if (!error) return items;
      const last = items[items.length - 1];
      if (last?.kind === "compaction" && last.running) {
        return [...items.slice(0, -1), { ...last, running: false, error }];
      }
      return items;
    }

    default:
      return items;
  }
}

// ---- live LiveEvent（只消费 AssistantStream；Durable 包装由 durable 帧承载，跳过防重复） ----

function applyLive(items: ChatItem[], raw: unknown): ChatItem[] {
  const ev = asRecord(raw);
  if (!ev) return items;
  if (ev.Durable !== undefined || ev.durable !== undefined) return items;
  // subagent 生命周期（瞬时）：start 不产生条目（子代理过程在父会话已有工具行），
  // end 仅在非正常收束（aborted / error / refusal / max-tokens）时补一行提示。
  const subagentEnd = asRecord(ev.SubagentEnd ?? ev.subagent_end);
  if (subagentEnd) {
    const name = asString(subagentEnd.name);
    const stop = asString(subagentEnd.stop_reason);
    if (stop && stop !== "completed") {
      return [...items, { kind: "notice", tone: "error", text: `子代理 ${name} 结束（${stop}）` }];
    }
    return items;
  }
  if (asRecord(ev.SubagentStart ?? ev.subagent_start)) return items;
  const stream = ev.AssistantStream ?? ev.assistant_stream;
  if (stream === undefined || typeof stream === "string") return items; // "End" 等单元变体
  const chunk = asRecord(stream);
  if (!chunk) return items;

  const text = typeof chunk.Text === "string" ? chunk.Text : "";
  const thinking = typeof chunk.Thinking === "string" ? chunk.Thinking : "";
  const start = asRecord(chunk.ToolCallStart);
  const input = asRecord(chunk.ToolCallInput);

  if (text) {
    return withStreamingTurn(items, (t) => ({ ...t, answer: t.answer + text }));
  }
  if (thinking) {
    // dsh web ui 对齐：思考按段累积——段 i 先于步骤 i、末段先于回答。
    // 何时新开一段：已有分段数 == 已落步骤数 + 1（当前段仍在「步骤 i 之后、
    // 步骤 i+1 之前」的空隙内）则续写末段，否则开新段。
    return withStreamingTurn(items, (t) => {
      const segs = t.thinkingSegs;
      const last = segs.length - 1;
      const next =
        segs.length === t.steps.length + 1
          ? [...segs.slice(0, last), segs[last] + thinking]
          : [...segs, thinking];
      return { ...t, thinkingSegs: next };
    });
  }
  if (start) {
    const step: ToolStep = {
      id: asString(start.id),
      name: asString(start.name),
      inputText: "",
      running: true,
    };
    return withStreamingTurn(items, (t) => ({
      ...t,
      steps: t.steps.some((s) => s.id === step.id && step.id !== "") ? t.steps : [...t.steps, step],
    }));
  }
  if (input) {
    const index = asNumber(input.index);
    const partial = asString(input.partial_json);
    return withStreamingTurn(items, (t) => ({
      ...t,
      steps: t.steps.map((s, i) => (i === index ? { ...s, inputText: s.inputText + partial } : s)),
    }));
  }
  return items;
}

// ---- 入口 ----

/** 历史回放：durable 事件数组（GET /v1/sessions/{id}/messages）→ 条目流。
 *  与快照帧共用同一聚合规则，供会话导出使用。 */
export function framesToItems(events: unknown[]): ChatItem[] {
  const items = events.reduce<ChatItem[]>(applyDurable, []);
  // 快照是完整历史：出口仍 running 的步骤必然是进程重启打断的中断残留
  // （tool_call 已落盘、tool_result 永不到达），收敛为「已中断」终态，
  // 否则重放界面将永远显示「运行中…」。
  return items.map((it) => {
    if (!isTurn(it) || !it.steps.some((s) => s.running)) return it;
    return {
      ...it,
      steps: it.steps.map((s) =>
        s.running
          ? { ...s, running: false, isError: true, resultText: s.resultText || "[会话中断，无返回]" }
          : s,
      ),
    };
  });
}

/** 消费一帧 SessionNotice；待回填帧由 ChatView 处理，其余渲染帧聚合为条目流。 */
export function applyFrame(items: ChatItem[], frame: AgentEventFrame): ChatItem[] {
  switch (frame.type) {
    case "snapshot": {
      const events = Array.isArray(frame.events) ? (frame.events as unknown[]) : [];
      return events.reduce(applyDurable, items);
    }
    case "durable":
      return applyDurable(items, frame.event);
    case "live":
      return applyLive(items, frame.event);
    case "turn_report":
      // 轮次收束摘要 → 末尾轮次块的 notice 行（非轮次场景静默跳过）
      return withLastTurn(items, (t) => ({ ...t, report: turnReportFromFrame(frame) }));
    default:
      return items;
  }
}
