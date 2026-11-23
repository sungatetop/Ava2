---
title: chatapp 渲染层设计
description: chatapp（对话 App）将 onion 事件流聚合为对话 UI 的渲染模型：帧→条目映射、轮次块聚合不变量、审批卡片交互、notice/操作行与会话切换。
keywords: chatapp, 渲染, frame-reducer, SessionNotice, 轮次块, 审批卡, 会话切换
author: AI 助手
date: 2026-11-08
updated: 2026-11-08
status: done
category: design
---

# chatapp 渲染层设计

chatapp 是 AppHost 契约的首个试点 App（`apps/chatapp/`），渲染采用「过程与最终回答分离」的对话形态：过程（thinking 折叠 + 工具步骤行）与最终回答（markdown）分列。本文说明其渲染层的关键设计。

## 1. 渲染模型（chat-types.ts）

对话内容聚合为三类条目：

| 类型 | 形状 | 说明 |
| --- | --- | --- |
| `UserItem` | `{ kind:"user", text, optimistic? }` | 用户气泡；`optimistic` 为发送即显的乐观气泡 |
| `TurnItem` | `{ kind:"turn", turn, thinking, steps[], answer, streaming, retryText?, report? }` | 轮次块：思考文本 + 工具步骤 + markdown 回答 |
| `PendingCard` | `{ requestId, kind:"approval"|"plan_review"|"question", tool?, input?, plan?, callId?, questions? }` | 待人工输入卡片（审批 / 计划评审 / 问询） |

`ToolStep`（轮次块内步骤行）：`{ id, name, inputText, running, isError?, resultText? }`；`TurnReport`（notice 行载荷）：`{ turn, steps, reason }`。

## 2. 帧 → 条目映射（frame-reducer.ts）

纯函数 reducer：`applyFrame(prev, frame) → next`，ChatView 以 `setItems(prev => applyFrame(prev, frame))` 消费。onion 事件契约见 `crates/onion-server/src/session.rs`。

| 帧 | 来源 | 渲染动作 |
| --- | --- | --- |
| `snapshot {events}` | 订阅建立 | 逐条走 durable 分支，重放历史 |
| `durable` `turn_start` | SessionEvent | `openTurn(turn)` 打开/复用轮次块 |
| `durable` `user_message` | SessionEvent | `source` 非空（宿主注入/nudge）不渲染；否则从后向前查找同文本乐观气泡就地转正，未命中则追加 |
| `durable` `assistant_message` | SessionEvent | 写入 `answer`，**不**置 `streaming:false`（收束仅由 turn_end 承担） |
| `durable` `assistant_attempt` | SessionEvent | 轮次块 `retryText`（模型失败重试提示行） |
| `durable` `tool_call` | SessionEvent | 按 `ToolCall.id` 去重后追加/更新步骤行，采纳 durable 全量输入（权威） |
| `durable` `tool_result` | SessionEvent | 按 `tool_call_id` 配对，写 `resultText` / `isError`，`running=false` |
| `durable` `turn_end` | SessionEvent | `streaming=false`（轮次收束唯一途径） |
| live `AssistantStream` | StreamChunk | `Text`→answer 增量、`Thinking`→thinking 增量、`ToolCallStart/ToolCallInput`→步骤行占位与 partial 输入、`End`→无操作 |
| live `Durable` | LiveEvent | 跳过（避免与 durable 帧双投递重复） |
| notice `approval_requested` / `plan_review_requested` / `question_requested` | SessionNotice | `requestId` 去重后加入卡片区（ChatView 持有 pending state，不进 items） |
| notice `turn_report` | SessionNotice | 写入末尾轮次块 `report`（notice 行数据源），同时更新 ChatView `lastTurn` state |
| notice `session_closed` | SessionNotice | 复位 busy（不进 items） |

## 3. 轮次块聚合的关键不变量

1. **事件顺序容忍**：`turn_start` durable 帧先于 `user_message` 到达，乐观气泡对账必须**从后向前**查找且越过已收束 turn 块即停（否则追加导致重复气泡）。
2. **工具行去重**：live `ToolCallStart` 先行占位 + durable `tool_call` 双投递——durable 分支按 id 去重，未命中才追加。
3. **收束唯一途径**：`assistant_message` 不结束轮次（中段回答后可能继续调工具）；仅 `turn_end` 置 `streaming:false`。live 流式增量落点为最后一个未收束块，否则隐式新开（`turn=-1`，收到 `turn_start` 后回填序号）。
4. **注入过滤**：`user_message.source` 非空 = 宿主注入（baseline/env/nudge），不渲染。RetryNudger 的失败纠正消息在 `crates/onion/src/listeners.rs` 侧打 `MessageSource::RuntimeContext` 标记，UI 与模型上下文语义解耦。

## 4. 组件结构（items.tsx）

对话视觉遵循 AppHost 设计系统：助手消息无气泡、无边框、无头像，过程与回答同列排布；用户消息为右对齐气泡。

```
ChatItems(items)                     # 条目流（无轮次卡底色、无「重新生成」）
├── UserBubble(text, optimistic?)    # 右对齐气泡：radius 20 / padding 10×16 / bg #EDF3FE（深 #2C2C2E）
├── AssistantTurn(item)              # 助手轮次：纯内容列，块间 gap 16
│   ├── RetryNote                    # 模型失败重试提示（assistant_attempt → retryText）
│   ├── ProcessBlock                 # 33px 折叠过程头（role=button + aria-expanded，默认折叠）
│   │   ├── ReasoningRow             # 思考文本，收起时显示首行摘要
│   │   └── ToolRow[]                # 24px 工具行（状态点 + 名称 · 摘要），展开显示输入/结果
│   ├── Markdown(answer)             # react-markdown + remark-gfm，代码块带语言条与复制
│   ├── 流式光标                       # streaming 时末尾块级光标
│   └── 操作行                        # 仅「复制」（15px 线性图标），hover 由 opacity 0→1
├── ApprovalCardView                 # warn 顶条 + 右对齐「拒绝 / 批准」→ replyApproval
├── PlanReviewCardView               # 无边框 + warn 条 → replyPlanReview
└── QuestionCardView                 # 单选/多选/自定义输入 → answerQuestion
```

- **过程头文案**：进行中为「处理中…」；收束后为「已完成 · N 步 · reason」（数字 `tabular-nums`），折叠体承载思考与工具行。
- **工具状态**：改用状态点表达（done 绿 / error 红 / running 旋转环 / idle 灰），不再用文字标签。
- **复制**：仅助手正文提供 `navigator.clipboard.writeText`；不提供「重新生成」按钮——重发由用户重新输入（`assistant_attempt` 仍以 RetryNote 提示）。
- **样式注入**：全部样式为 `styles.ts` 导出的 `chatappCss` 字符串，由 ChatView 在根节点内以 `<style>` 注入，且统一限域在 `.cc-root` 下；避免 `setup()` 直接操作 DOM（契约要求 setup 只登记 slot 与订阅）。令牌按设计系统数值落 `.cc-root` 局部变量，并在 `[data-theme="dark"]` 下覆盖，跟随宿主主题。

## 5. 会话装配与切换（chat-view.tsx）

- **挂载**：`listSessions()` 填充下拉并 `attach(第一个)`；列表为空显示「新会话（发送后创建）」占位。
- **attach(id)**：幂等守卫（同 id 直接返回）→ dispose 旧订阅 → `onSessionEvent` 订阅（宿主每会话单条 WS 多路复用）→ **重置 items/pending/busy/error/lastTurn** → `getPending` 存量卡片拉取 + `getSession` 拉取 `lastTurn`。切换后由 snapshot 帧重放历史。
- **发送**：`send(override?)`——乐观气泡先行，`ensure()` 复用当前会话或 `createSession`；失败回滚乐观气泡并显示错误。
- **重放 notice 补齐**：`turn_report` 是 live 帧**不落盘**，快照重放天然缺失。ChatView 用 REST `getSession(id).lastTurn` 补齐：effect 监听 `items + lastTurn`，当末尾条目为收束轮次块、无 `report` 且 `turn` 号与 `lastTurn.turn` 一致时写入。live 场景（轮次收束时 turn_report 帧到达）由 reducer 直接写 `report`，两条路径互不冲突。
- **新建会话**：`createSession` → attach（attach 内部将 id 并入下拉）。

## 6. 宿主通道依赖

App 经 `ctx.agent`（AgentChannel，`src/host/agent.ts`）访问：`listSessions` / `getSession` / `createSession` / `send` / `getPending` / `replyApproval` / `replyPlanReview` / `answerQuestion` / `onSessionEvent`。事件帧为宿主透传的 onion `SessionNotice`（松散 JSON），chatapp 不直连 WS。

## 7. 图片进入上下文的两层预算

图片经工具结果以标准图片块进入多模态输入，需两层预算保证正常截图不被上下文压缩卸载：

- **层 1 · 进上下文前预算**（`onion/src/media.rs`，`read_image` 返回前）：像素 >1.15MP → 长边 1568 + 面积比例缩放重编码（alpha 走 PNG / 其余 JPEG q85；重编码不小于原图则保留原图，失败降级不失败请求）；字节 >1MB → 压缩重编码；解码失败/损坏 → 文本占位。请求视图另以 `evict_old_images` 封顶历史图片 ≤4 张（kernel 装配先于 prune_view 应用）。transcript/state 原件不动。
- **层 2 · 估算封顶**（`compaction/estimate.rs::IMAGE_TOKEN_ESTIMATE = 2048`）：图片按预算后固定单图估算，不再按 data URL 字符数折算——估算面向预算后的请求视图而非原始字节。

配套约束：`llm.rs` 的 `wire_message` 在 Tool 结果含 Image 块时 content 必须改用 parts 数组（`text` + `image_url`），纯文本结果保持字符串形态——否则图片在请求装配层被静默丢弃，模型会基于「成功但无内容」的工具结果编造描述。
