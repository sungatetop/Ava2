// chatapp 渲染模型：从 onion 事件流聚合出的对话条目。
// 设计对齐 ava-app：过程（thinking / 工具步骤）与最终回答分离——
// 用户气泡 + 「轮次块」（thinking 折叠区 + 工具步骤行 + markdown 回答）。

export interface ToolStep {
  /** tool call id（onion ToolCall.id / ToolResult.tool_call_id 配对键） */
  id: string;
  name: string;
  /** 输入预览（durable tool_call 的权威 JSON；live 期为 partial_json 累积） */
  inputText: string;
  running: boolean;
  isError?: boolean;
  resultText?: string;
}

/** 单轮 token 用量（轮次内各步 LLM usage 累计，snake_case 帧 → camelCase） */
export interface TurnUsage {
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheWriteTokens: number;
}

/** turn_report 帧载荷（轮次收束摘要，渲染为轮次块底部 notice 行）。 */
export interface TurnReport {
  turn: number;
  steps: number;
  reason: string;
  /** 该轮累计 token 用量（缺省视为全 0） */
  usage?: TurnUsage;
}

/** 一个轮次块：流式开始（live Start / durable turn_start）到回答收束（assistant_message / turn_end）。 */
export interface TurnItem {
  kind: "turn";
  /** onion turn 序号；live 隐式创建（未收到 turn_start）时为 -1 */
  turn: number;
  /** 思考分段（dsh web ui 对齐）：segs[i] 先于 steps[i]，末段先于回答；
   *  不变量 thinkingSegs.length ≤ steps.length + 1，流式时随 chunk 逐段追加 */
  thinkingSegs: string[];
  steps: ToolStep[];
  answer: string;
  /** true = 轮次进行中（assistant_message / turn_end 收束为 false） */
  streaming: boolean;
  /** 各步 assistant_message 携带的 usage 累加（回放态没有 turn_report 时的兜底数据源） */
  usage?: TurnUsage;
  /** 模型请求重试（durable assistant_attempt） */
  retryText?: string;
  /** 轮次收束摘要（SessionNotice turn_report） */
  report?: TurnReport;
}

/** 用户消息里的图片附件（发送时为 dataUrl；回放时取自 user_message 的 image 块） */
export interface UserAttachment {
  name: string;
  mimeType: string;
  /** data URL（`data:image/png;base64,...`） */
  dataUrl: string;
}

export interface UserItem {
  kind: "user";
  text: string;
  /** 图片附件（可空；纯图片消息的 text 为空串） */
  attachments?: UserAttachment[];
  /** 乐观气泡（发送即显，durable user_message 回显后转正） */
  optimistic?: boolean;
}

/** 斜杠命令本地回执（不投递给 agent，仅消息流展示；tone 区分成败） */
export interface NoticeItem {
  kind: "notice";
  tone: "info" | "error";
  text: string;
}

/** 斜杠命令生命周期：右对齐回显气泡 + 结果行。
 *  边车只落盘 command_run / command_done 帧；chatapp 的斜杠命令在本地执行，
 *  故由前端合成同一形态的条目。 */
export interface CommandItem {
  kind: "command";
  /** 条目 id（执行中 → 终态就地回填的定位键） */
  id: string;
  /** 命令名（不含前导 `/`） */
  name: string;
  /** 参数原文（含前导空格，可为空串） */
  args: string;
  /** null = 执行中；否则为终态 */
  outcome: { kind: "success" | "error"; text: string } | null;
  /** 本命令产生的首条领域事件下标：
   *  有值时该命令由富卡片承载（压缩标记 / 目标横幅 / 计划状态），
   *  回执文本降级为不渲染。 */
  sourceEventSeq?: number | null;
}

/** 上下文压缩标记：durable compaction_summary 投影。 */
export interface CompactionItem {
  kind: "compaction";
  /** 压缩摘要正文（可展开；null = 摘要不可用） */
  summary: string | null;
  /** 被替换的历史条目数（null = 结构化计数不可用） */
  shadowedItemCount: number | null;
  /** 被替换的 token 数 */
  shadowedTokenCount: number | null;
  /** true = 压缩事务进行中（compaction_start 已到、summary 未到） */
  running?: boolean;
  /** 压缩失败原因（compaction_end.error） */
  error?: string;
}

export type ChatItem = UserItem | TurnItem | NoticeItem | CommandItem | CompactionItem;

/** 待人工输入卡片（approval_requested / plan_review_requested / question_requested）。 */
export interface PendingCard {
  requestId: string;
  kind: "approval" | "plan_review" | "question";
  tool?: string;
  input?: unknown;
  /** 审批来源标记（`sandbox-retry` = 沙箱拒绝后的越界执行申请；缺省 = 首次审批） */
  reason?: string;
  plan?: string;
  callId?: string;
  questions?: unknown[];
}
