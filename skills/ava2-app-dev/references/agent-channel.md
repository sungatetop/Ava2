# AgentChannel 完整方法清单

`ctx.agent` 是平台 Agent 通道（REST + WS 收敛封装，每会话一条 WS 多路复用，引用归零即关闭）。App **不直连 WS**，一律经本通道。以下按域分组，签名为主，语义要点随后。

## 会话

| 方法 | 签名 | 语义 |
| --- | --- | --- |
| `listSessions` | `(): Promise<{ sessions: SessionInfo[] }>` | 返回顺序未定义，勿依赖；列表项不含 `model` / `approvalPreset`，需要时走 `getSession(id)` 详情 |
| `getSession` | `(id): Promise<SessionInfo>` | 会话详情 |
| `createSession` | `(cwd?): Promise<SessionInfo>` | `cwd` 即工作区：权限根在会话创建期固定，跨工作区 = 新会话 |
| `deleteSession` | `(id): Promise<void>` | 删除会话 |
| `onSessionEvent` | `(id, fn): Disposable` | 订阅会话事件（原样透传引擎 `SessionNotice`）；**首个 `snapshot` 帧既是全量历史也是连接就绪信号** |
| `getMessages` | `(id): Promise<SessionEvent[]>` | 拉取历史事件 |
| `send` | `(id, text, model?, directive?): Promise<void>` | 发送用户输入 |
| `interrupt` | `(id): Promise<void>` | 打断当前轮次 |
| `compact` | `(id): Promise<void>` | 压缩上下文 |

## 工作区文件

| 方法 | 签名 | 语义 |
| --- | --- | --- |
| `listDirs` | `(): Promise<...>` | 可用工作目录列表 |
| `readWorkspaceFile` | `(path): Promise<string>` | 读当前工作区文件 |

## 模型 / 供应商

| 方法 | 签名 | 语义 |
| --- | --- | --- |
| `listModels` | `(): Promise<{ defaultModel, providers[] }>` | 供应商分组模型 |
| `addModel` / `removeModel` / `setDefaultModel` | 管理 provider 内模型 | |
| `addProvider` / `removeProvider` / `testModel` | 管理供应商 / 连通性测试 | |
| `setSessionModel` | `(id, model): Promise<SessionInfo>` | 会话运行期切换模型，返回更新后的 SessionInfo |
| `setApprovalPreset` | `(id, preset): Promise<SessionInfo>` | preset ∈ `workspace-write` / `auto` / `danger-full-access` |

## 待回填（审批 / plan 评审 / 问询）

| 方法 | 签名 | 语义 |
| --- | --- | --- |
| `getPending` | `(id): Promise<{ kind: "approval" \| "plan_review" \| "question", ... }>` | 统一待回填表 |
| `replyApproval` | `(id, requestId, decision: "approve" \| "deny"): Promise<void>` | 工具审批答复 |
| `replyPlanReview` | `(id, requestId, verdict: "approve" \| "keep_planning" \| "dismissed"): Promise<void>` | plan 裁决 |
| `answerQuestion` | `(id, requestId, answers: AnswerInput[]): Promise<void>` | 人工问询答复（AnswerInput 支持 custom 自由输入） |

## 待办 / goal / plan

| 方法 | 签名 | 语义 |
| --- | --- | --- |
| `getTodos` | `(id): Promise<TodoItem[]>` | 待办清单；TodoItem = `{ content, status: "pending" \| "in_progress" \| "completed" }` |
| `getGoal` | `(id): Promise<GoalInfo \| null>` | goal 快照；未设置时 null；GoalInfo 含 `revision`（乐观锁）、`phase`（active/paused/blocked/complete）等 |
| `setGoal` | `(id, input: SetGoalInput): Promise<GoalInfo \| null>` | goal 创建/编辑；已存在时必须带 `revision` |
| `goalAction` | `(id, action, revision, reason?): Promise<GoalInfo \| null>` | 生命周期动作：`pause` / `resume` / `complete` / `block` / `clear` |
| `getPlan` | `(id): Promise<PlanInfo>` | plan 模式状态 `{ active, pending }`；pending 非空表示切换将在当前轮次步骤边界生效 |
| `setPlan` | `(id, active): Promise<PlanInfo>` | plan 模式开关；busy 时 `pending=true` 延迟生效 |

## 任务

| 方法 | 签名 | 语义 |
| --- | --- | --- |
| `listTasks` / `createTask` / `toggleTask` / `deleteTask` / `runTask` | 任务 CRUD 与执行 | 任务为后台作业一等公民 |

## 智能体

| 方法 | 签名 | 语义 |
| --- | --- | --- |
| `listAgents` | `(): Promise<AgentInfo[]>` | 可用智能体列表 |
