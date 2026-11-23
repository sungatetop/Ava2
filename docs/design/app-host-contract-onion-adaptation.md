---
title: AppHost 契约适配 onion 的差异分析与待决问题
description: 以 ava-app 冻结版 AppHost 契约为基线，逐条比对 onion-server 的 REST/WS 能力面，给出「可直接复用 / 需适配 / 需新增 / 应删除」四类判定与待决问题清单，作为 ava2 宿主内核落地前的讨论底稿。
keywords: AppHost, app-contract, onion-server, 契约适配, AgentChannel, 差异分析, ava2
author: AI 助手
date: 2026-11-07
status: review
category: design
---

# AppHost 契约适配 onion 的差异分析

## 1. 背景与目标

ava 的 AppHost 契约（[app-contract.md](../../../ava/docs/ava-app/design/app-contract.md)，冻结版）
是围绕 **ava-server** 的端点形态设计的。ava2 的 Agent 运行时换成 **onion-server**，
两者端点集合、事件模型、会话语义都不同，因此契约**不能原样搬运**。

本文只做**差异盘点与判定**，不写实现。目标是把「哪些能直接复用、哪些要改、哪些要新增、
哪些该删」摆清楚，供讨论收敛后再落宿主内核。

## 2. 契约面总览（五份清单）

| 清单 | 内容 | 与 onion 的耦合度 |
| --- | --- | --- |
| 一 | `setup(ctx)` 签名 + `AppContext` 注出面 | 低（纯宿主侧） |
| 二 | manifest 声明面（`inject` / `surfaces` / `config` / `contributes`） | 低（纯宿主侧） |
| 三 | `ctx.agent` 能力清单（AgentChannel） | **高**（逐条绑端点） |
| 四 | 总线事件（`canvas.open` / `agent.invoke` 等） | 无（宿主内部） |
| 五 | 能力贡献契约（`contributes`：skills/mcpServers/agents/hooks/apps） | **高**（绑 ava-server plugin 模块） |

结论先行：**清单一二四可原样复用**；**清单三需重写**；**清单五需重新定义落点**。

## 3. 清单三：AgentChannel 逐条比对

### 3.1 端点映射表

| 契约方法 | ava-server 端点 | onion-server 现状 | 判定 |
| --- | --- | --- | --- |
| `onSessionEvent(id, fn)` | `WS /ws/session/{id}` | `WS /v1/sessions/{id}/ws` | **适配**（路径 + 帧格式） |
| `getMessages(id)` | `GET /v1/sessions/{id}/messages` | 同路径，返回 `SessionEvent[]` | **适配**（帧结构不同） |
| `listSessions()` | `GET /v1/sessions` → 数组 | 同路径 → `{ sessions: [...] }` | **适配**（包了一层 + 字段不同） |
| `createSession(workspacePath?)` | `POST /v1/sessions` | 同路径，body 为 `{ id?, agent?, interactive?, cwd? }` | **适配**（`workspace_path` → `cwd`） |
| `deleteSession(id)` | `DELETE /v1/sessions/{id}` | 同路径 | **复用** |
| `send(id, text, model?, directive?)` | `POST /v1/sessions/{id}/send` | `POST /v1/sessions/{id}/messages`（`parts[]` + `wait`） | **适配**（路径 + body 形态） |
| `setModelSelection(id, model)` | `PUT /v1/sessions/{id}/model-selection` | **无** | **新增**（或删除） |
| `listWorkspaces()` | `GET /v1/workspaces` | **无**（工作区概念 = 会话 `cwd`） | **删除 / 重定义** |
| `createWorkspace(path)` | `POST /v1/workspaces` | **无** | **删除 / 重定义** |
| `setActiveWorkspace(path)` | `PUT /v1/workspaces/active` | **无** | **删除 / 重定义** |
| `listModels()` | `GET /v1/models` → `{models, default_model}` | `GET /v1/models` → `{defaultModel, providers[]}` | **适配**（结构不同） |
| `listAgents()` | `GET /v1/agents` → 数组 | `GET /v1/agents` → 数组（`{id,name,tools}`） | **复用** |
| `getSessionAgent(id)` | `GET /v1/sessions/{id}/agent` | 同路径 → `{active, agents[]}` | **适配**（取 `active`） |
| `setSessionAgent(id, type)` | `POST …/agent` | `PUT …/agent`（body `{agentId}`） | **适配**（方法 + body） |
| `searchFiles(q, ws?)` | `GET /v1/files/search` | **无** | **新增** |
| `uploadAttachment(file)` | `POST /v1/uploads` | **无** | **新增** |
| `interrupt(id)` | `POST …/interrupt` | 同路径 | **复用** |
| `compact(id)` | `POST …/compact` | 同路径 | **复用** |
| `getApprovals(id)` | `GET …/approvals` | `GET …/pending`（统一待回填表） | **适配**（模型更统一） |
| `replyApproval(...)` | `POST /v1/approvals/reply` | `POST …/approvals/{requestId}` | **适配**（requestId 进路径） |
| `planReply(...)` | `POST /v1/plan/reply` | `POST …/plan-reviews/{requestId}` | **适配** |
| `answerQuestion(...)` | `POST /v1/questions/answer` | `POST …/questions/{requestId}` | **适配** |
| `getApprovalTier(id)` | `GET …/approval-tier` | **无**（档位是全局 `approval_preset`） | **删除 / 重定义** |
| `setApprovalTier(id, tier)` | `POST …/approval-tier` | **无** | **删除 / 重定义** |
| `getWorkflowState(id)` | `GET …/workflow-state` | **无**（plan 轴为 `GET/POST …/plan`） | **适配**（映射到 plan） |
| `setWorkflowState(id, wf)` | `POST …/workflow-state` | **无** | **适配**（映射到 plan） |

### 3.2 事件模型差异（最关键）

ava-server 的 WS 帧是**投影后的 UI 事件**（`messages_state` / `message_upserted` /
`assistant_delta` / `run_end`），前端拿到即可渲染。

onion-server 的 WS 帧是**引擎原始事件**，`SessionNotice` 八种：

| `type` | 载荷 | 语义 |
| --- | --- | --- |
| `snapshot` | `{ events: SessionEvent[] }` | 连接首帧，durable 全量（重连对齐） |
| `durable` | `{ ts, event: SessionEvent }` | 落盘事件（TurnStart / UserMessage / AssistantMessage / ToolCall / ToolResult / TurnEnd / Compaction* / Hook* …） |
| `live` | `{ event: LiveEvent }` | 瞬时事件（`StateChanged` / `Inbox*` / `AssistantStream(StreamChunk)`） |
| `approval_requested` | `{ requestId, tool, input }` | 待审批 |
| `plan_review_requested` | `{ requestId, plan }` | 待 plan 裁决 |
| `question_requested` | `{ requestId, callId, questions }` | 待人工问询 |
| `turn_report` | `{ turn, steps, reason }` | 轮次结束 |
| `session_closed` | `{ reason }` | 会话关闭 |

**这意味着宿主必须承担一层「引擎事件 → UI 投影」**，而 ava 里这层在 ava-server 内部。
这是 ava2 最大的架构决策点（见 §5 问题 Q1）。

### 3.3 会话语义差异

| 维度 | ava-server | onion-server |
| --- | --- | --- |
| 会话创建 | 显式创建，立即落盘 | 显式创建；`interactive` 缺省 true |
| 工作区 | 独立 workspace 注册表 + active 概念 | 无注册表，会话 `cwd` 即工作区；`ONION_SERVER_WORKSPACE` 兜底 |
| 会话列表 | 数组，含 title/时间 | `{sessions[]}`，live 与磁盘合并，含 `live` / `eventCount` / `goalPhase` |
| 消息发送 | `/send` 纯文本 + model + directive | `/messages` 内容块数组（text/file/data）+ `wait` 同步开关 |
| 模型选择 | 会话级持久化 | 全局 `Config.model` + `ModelTargetRef`（会话内可换，但无 REST 端点） |
| 审批档位 | 会话级 tier | 全局 `approval_preset` |
| 任务/作业 | 无 | `tasks` / `jobs` 一等公民（A2A 语义） |
| goal / plan | 无 | 一等公民（`goal` / `plan` 端点族） |

## 4. 清单五：contributes 的落点问题

ava 的 `contributes` 契约**寄生于 ava-server 的 plugin 模块**（ADR-011 D2）：
`GET /v1/plugins` 发现、`/v1/plugins/{id}/enabled` 启停、`/v1/plugins/{id}/config` 存储。

**onion-server 完全没有 plugin 概念**。因此 ava2 有三条路：

| 方案 | 做法 | 代价 |
| --- | --- | --- |
| A. 宿主自持 | App 清单与启停状态全落宿主本地（文件 / localStorage），不依赖后端 | 与「plugin 即 app」的运行时语义脱钩；能力贡献（skills/mcp/agents）无法真正注入引擎 |
| B. onion-server 新增 plugin 模块 | 在边车里实现发现/启停/配置 + 贡献注入 | 改动边车（当前是 ava 的复制品，会分叉） |
| C. 宿主声明 + 边车消费 | 宿主把 `contributes` 归一成 onion 的既有配置形态（`agents.yml` / `mcp.json` / skills 目录 / hooks 配置），经会话创建时注入 | 需要 onion 侧支持「按会话注入配置」，当前是进程级装配 |

**倾向 C**：onion 的装配是进程级（`Config` + `assemble`），但会话有 `cwd`，
skills / agents / mcp 都支持工作区级目录（`.onion/`）。若 App 的贡献物落到工作区目录，
引擎天然可见，无需新增后端模块。**待确认**（Q4）。

## 5. 待决问题清单

| # | 问题 | 选项 | 影响面 |
| --- | --- | --- | --- |
| Q1 | 引擎事件 → UI 投影放哪？ | (a) 宿主前端投影（`src/host/projection.ts`）；(b) onion-server 新增投影层；(c) 混合：durable 前端投影、live 直通 | 决定 `AgentChannel` 是否要重写、前端复杂度 |
| Q2 | 工作区概念怎么落？ | (a) 删除 workspace API，App 直接用会话 `cwd`；(b) 宿主维护 workspace 列表（本地文件），创建会话时传 `cwd` | 影响 chat / workspace 两个预置 App |
| Q3 | 模型选择粒度？ | (a) 删除会话级选择，只读全局 `GET /v1/models`；(b) onion-server 新增 `PUT /v1/sessions/{id}/model` | 影响 ModelPill 交互 |
| Q4 | `contributes` 落点？ | 见 §4 的 A / B / C | 决定是否改边车 |
| Q5 | 审批档位？ | (a) 删除（全局 preset 只读展示）；(b) 宿主本地档位 + 前端过滤审批请求 | 影响审批 UI |
| Q6 | 附件上传 / 文件搜索？ | (a) 宿主侧实现（Tauri fs + 本地索引）；(b) onion-server 新增端点 | 影响 `#` mention 与附件 chip |
| Q7 | `send` 的 `directive`（斜杠命令）？ | (a) 删除；(b) 宿主解析后转成 `parts` 里的 data 块 | 影响 `/compact` 等命令 |
| Q8 | 会话标题？ | onion 的 `SessionSummary.title` 来自首条用户消息；是否需要宿主侧重命名 | 影响会话列表 |
| Q9 | goal / plan / tasks / jobs 是否进契约？ | (a) 不进（App 不可见）；(b) 新增 `ctx.agent` 方法暴露 | 决定 ava2 是否比 ava 能力更强 |
| Q10 | 边车分叉策略？ | (a) ava2/crates 独立演进；(b) 定期从 ava 同步；(c) 抽公共 crate | 长期维护成本 |

## 6. 建议的收敛路径

1. **先定 Q1**（投影层位置）——它决定 `AgentChannel` 的形状，其余问题都依赖它。
2. **Q2 / Q3 / Q5 / Q7 倾向「删减」**：ava2 是原型落地，契约面越小越容易验证；
   缺失能力后续按需加回，而不是一开始就补齐 ava 的全部端点。
3. **Q4 倾向方案 C**（宿主声明 + 工作区目录消费），避免边车分叉。
4. **Q9 倾向 (b)**：goal / plan / tasks / jobs 是 onion 相对 ava 的增量能力，
   原型阶段就暴露出来，才能验证「AppHost 能否承载更丰富的 Agent 语义」。

## 7. 下一步

- 本文经讨论收敛后，产出 `design/app-host-host-kernel.md`（宿主内核设计：registry / loader / projection / slot）。
- 再产出 `solutions/agent-channel-onion.md`（`AgentChannel` 的 onion 版逐条签名）。

## 8. 演进预留

- **manifest 展示级字段（2026-11-11 已落地 `author` / `homepage` / `license` / `icon` 四个可选字段**，详见 `solutions/app-manifest-developer-info.md`）：均为与 `description` 同级的可选展示字段，hostApi 不升级、缺省兼容；`icon` 为应用形象的声明式权威（`appIconFor` 优先 manifest，slot 遍历降级为兜底）。
- 结构化 `developer` 对象、`repository` 等为未来扩展位：届时 `author` 保留为兼容别名（读取顺序 `developer?.name ?? author`）。
- 开发者签名 / 身份认证属信任体系专题，待分发渠道定型后另行设计。
