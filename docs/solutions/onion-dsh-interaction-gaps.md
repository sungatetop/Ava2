---
title: onion 与 dsh 交互逻辑差异清单
description: 逐项对比 ava2 onion（Rust 内核 + 边车）与 deepseek-harness（dsh）在斜杠命令、subagent 委派、plan/goal/compact、审批与 ask_user、会话事件模型五个契约面的交互差异，标注影响等级与修复优先级。
keywords: onion, dsh, deepseek-harness, 交互逻辑, 差异对比, subagent, 斜杠命令, 审批, 事件模型
author: AI 助手
date: 2026-11-14
updated: 2026-11-15
status: review
category: solutions
---

# onion 与 dsh 交互逻辑差异清单

## 背景

ava2 的 Agent 运行时（`crates/onion` 内核 + `crates/onion-server` 边车）以 deepseek-harness（下称 dsh）为对齐基准。本文记录一次系统性交互逻辑对比的结论，作为后续修复的输入。

对比范围：斜杠命令、subagent 委派、plan/goal/compact、审批与 ask_user、会话事件模型。

影响等级：**高** = 有运行时故障证据或阻断能力；**中** = 能力缺失但可绕过；**低** = 风格/边界差异。

## 一、斜杠命令

| # | 差异点 | dsh | onion | 等级 |
|---|---|---|---|---|
| 1 | 命令目录规模 | 6 个内置：goal/plan/feedback/compact/permission/export，含中英别名（`packages/interaction/commands/src/builtin.ts:4-11`） | 服务端 5 个：compact/goal/plan/feedback/permission（`crates/onion-server/src/rest/commands.rs`）；`/export` 为 chatapp 本地命令（复用侧边栏导出，无需服务端路由）；前端另有 5 个本地命令 help/new/stop/model/approval；~~中英别名未做~~ → 已补中文别名 | 中 |
| 2 | 命令执行位置 | Host 进程内本地执行，不发给模型 | 已对齐：服务端执行 + 前端本地命令分流（`apps/chatapp/src/chat-view.tsx:821-906`） | — |
| 3 | 命令落盘 | `command/run` + `command/done` 成对 log-only，`commandId` 配对 | 已对齐（`rest/commands.rs:44-56`） | — |
| 4 | 富渲染指针 `sourceEventSeq` | `command/done` 携带 `sourceEventSeq` 指向权威领域事件，客户端据此渲染富卡片而非回执文本（`packages/interaction/commands/src/types.ts:112`） | ~~缺失：`CommandDone` 只有 `kind` + `text`~~ → 已补 `source_event_seq` + 前端富卡片 | 高 |
| 5 | 命令名正则 | `^[a-z][a-z0-9_-]*$` 严格校验（`commands/src/index.ts:32`） | ~~仅 `trim_start_matches('/').to_lowercase()`~~ → 已补 `valid_name()` 校验（400 `invalid_command_name`） | 低 |
| 6 | 未知命令 | 不落盘 | ~~落盘 CommandRun + CommandDone~~ → 已对齐（只回执 `kind:"error"`，不落盘） | 低 |
| 7 | 命令作用域分层 | `ScopedLayers` 全局 + agent 作用域覆盖（`commands/src/index.ts:264`） | 无分层，单一命令表（onion 无插件注册面：命令是宿主内置的 5 个固定能力，不存在「某 agent 私有命令」的生产者） | 低 |

## 二、subagent 委派

| # | 差异点 | dsh | onion | 等级 |
|---|---|---|---|---|
| 8 | `cwd` 入参 | 委派工具支持 `cwd`（`packages/subagent/tool-subagent/src/index.ts:218-260`） | ~~未实现，子会话 cwd 固定父会话~~ → 已补 `SessionCwd::new_at` + 权限根校验 | 高 |
| 9 | `agent_preset` 入参 | 支持，且 toolFilter/persona 已废弃必须走 preset（`tool-subagent/src/index.ts:150-154`） | ~~无此字段，子代理工具组合硬编码~~ → 已补 `agent_preset` 白名单 + 预设工具面/指令 | 中 |
| 10 | `wait_agent` 工具 | 有（`tool-subagent-control/src/wait-agent.ts`） | ~~无，模型只能轮询 `list_agents`~~ → 已补 `wait_agent`（settle Notify 阻塞等待） | 高 |
| 11 | 路由白名单默认值 | 无 policy 时只允许父路由，policy 是 durable 事件且子会话继承（`tool-subagent/src/model-selection.ts:136-144`） | ~~目录即白名单~~ → 已补 `SubagentModelSelectionPolicy` durable 事件 + 继承 | 中 |
| 12 | 路由预检 | `preflightChildLlmRoute` 走 `llm.resolveCallConfig` 真实预检 | ~~仅目录静态校验，不预检连通性~~ → 已补 `preflight_route` 真实请求预检（仅显式路由） | 低 |
| 13 | name 规则 | `^[a-z][a-z0-9-]{0,31}$`，根名 `lead`，全名是 `/` 路径（`subagent/src/names.ts:16-47`） | 已对齐字符集/长度/lead 保留/唯一性；onion 子代理深度恒为 1（工具面不含 subagent 工具，天然不递归），故全名 ≡ 单段名，层级路径无生产者 | 低 |
| 14 | 状态枚举 | `running\|inactive\|corrupt\|unsupported\|unavailable`（`tool-subagent-control/src/list-agents.ts:12`） | ~~仅 `Running\|Settled`~~ → 已收敛 `Running\|Inactive`（诊断三态属下游子树不可读场景，扁平活跃表不产生） | 中 |
| 15 | settle 回投 source | `subagent-settled{form:'notice',summary,stopReason,senderSessionId}` 与 `agent-message{form:'relay'}` 严格区分（`subagent/src/continuation-messages.ts:14-39`） | ~~缺 `stopReason` 与 `summary`~~ → 已补双 source + 一行账目 | 中 |
| 16 | 投递策略 | 父 idle 用 `queue`、否则 `steer`（`subagent/src/manager.ts:1134`） | ~~统一 `inbox.steer`~~ → 已补 `Delivery::{Queue,Steer}` 按父会话空闲选择 | 低 |
| 17 | 子代理描述符 | durable `subagent/descriptor` v3，携带 provider/model/effort/persona/toolFilter（`subagent/src/descriptor.ts:38-86`） | ~~无 descriptor 事件~~ → 已补 `SubagentDescriptor`（写入子会话日志） | 中 |
| 18 | 生命周期事件 | `subagent/start` / `subagent/end` 为瞬时事件（`subagent/src/lifecycle.ts:25-29`） | ~~无对应 live 事件~~ → 已补 `LiveEvent::SubagentStart/End`（run_id 配对） | 中 |

## 三、plan / goal / compact

| # | 差异点 | dsh | onion | 等级 |
|---|---|---|---|---|
| 19 | goal 显式 edit | `/goal edit <objective>`，complete 时 edit 退化为 create（`packages/goal/command-goal/src/index.ts:154-164`） | 已对齐（`rest/commands.rs:128+`） | — |
| 20 | plan 无 edit | 只有 on/off + `exit_plan_mode` 审批 | 已对齐（`crates/onion/plan/service.rs:86-124`） | — |
| 21 | plan pending 折叠 | `pendingIntents` 在 open-turn 内挂起、step 边界追加 | 已对齐（`pending` + `flush_pending`，`plan/service.rs:86-97`） | — |
| 22 | compact 检查点消息 | 压缩后由紧随的 `user/message`（source `compact-checkpoint`）真正替换上下文（`packages/compaction/src/checkpoint.ts:19`） | ~~无 `compact-checkpoint` source~~ → 已补 `MessageSource::CompactCheckpoint` + 紧随 `CompactionSummary` 的 durable user 消息 | 中 |
| 23 | compact 事件集 | `compaction/start\|summary\|end\|prune` 四件 | 已对齐四件（`CompactionStart/Summary/End/Pruned`，`crates/onion/session.rs:37-45`） | — |
| 24 | compact 回执指针 | 成功回 `sourceEventSeq: result.summarySeq` | ~~仅文本回执（同 #4）~~ → 已补 `summary_seq` 精确指针 | 中 |

## 四、审批与 ask_user

| # | 差异点 | dsh | onion | 等级 |
|---|---|---|---|---|
| 25 | 审批必须 turn 内 | 非 turn 内直接抛错（`packages/interaction/user-approval/src/index.ts:244-250`） | ~~无此约束~~ → 已补 `approval::turn_open` 判据，轮次外 fail-closed 拒绝且不落审计 | 低 |
| 26 | 审批 outcome 闭集 | `allowed-once\|rejected\|cancelled\|unavailable` | ~~三态，无 `cancelled`/`unavailable`~~ → 已补 `ApprovalOutcome` 闭集 + 中断撤回 | 中 |
| 27 | 子代理审批种子 | durable `approval/policy` 带 `source:'delegation'` 标记 | ~~子代理共享父审批通道（不绕过），但无 delegation 来源标记~~ → 已补 durable `ApprovalPreset` + `source:delegation` 播种进子会话 | 低 |
| 28 | ask_user 超时 | `timed` 模式默认 120s，迟到回复以 `user-question-reply` 回投 | 支持 timeout（None/-1 无限阻塞）；~~无迟到回复回投通道~~ → 已补三宿主迟到注入 | 中 |

## 五、会话事件模型

| # | 差异点 | dsh | onion | 等级 |
|---|---|---|---|---|
| 29 | 事件命名风格 | 域前缀 + `/`，段内 kebab：`command/run`、`subagent/model-selection-policy` | 全 snake_case 单段：`command_run`、`goal_change`（`crates/onion/session.rs:20-124`） | 低（纯风格，**不修**） |
| 30 | surface 事件白名单 | 仅 5 类可带 `surfaceOp`，其余 `surfaceOp?: never`（`packages/core/session/src/types.ts:447-528`） | ~~无 surfaceOp 概念~~ → 已补 `SessionEvent::is_surface()` 白名单（4 类消息事件） | 低 |
| 31 | 未知事件兼容 | 必须带 `ignorable:true` 才可跳过，否则拒绝重建 | ~~无此机制（serde 反序列化失败即失败）~~ → 已补 `replay_event()` 严格回放（未知且未标 ignorable → 拒绝重建） | 中 |
| 32 | durable/live 分界 | 已对齐 | 已对齐（`append` vs `emit_live`，`crates/onion/session.rs:287-303`） | — |

## 修复优先级

1. **#8 `cwd` 委派字段** —— 唯一有运行时故障证据的缺陷（子代理路径解析错误）。
2. **#10 `wait_agent`** —— 缺失导致模型只能轮询，实测浪费 3 个 step。
3. **#4 `sourceEventSeq`** —— 缺失使 compact/goal/plan 无法渲染富卡片。
4. **#14 状态枚举 + #18 生命周期事件** —— 前端无法感知子代理失败/损坏。
5. ~~**#17 descriptor 落盘** —— 冷恢复后子代理元信息丢失。~~（已完成）
6. ~~**#15 settle 回投补 `stopReason`/`summary`**~~（已完成）
7. ~~**#16 投递策略** —— 父 idle 用 `queue`、运行中用 `steer`。~~（已完成）
8. ~~**#5 / #6 命令名校验 + 未知命令不落盘**~~（已完成）
9. ~~**#31 未知事件 ignorable**~~（已完成）
10. ~~**#26 审批 outcome 闭集**~~（已完成）；~~**#28 ask_user 迟到回复**~~（已完成）；~~**#24 compact 回执指针**~~（已完成）；~~**#27 委派审批种子**~~（已完成）；~~**#25 审批必须 turn 内**~~（已完成）；~~**#12 路由预检**~~（已完成）；~~**#30 surface 白名单**~~（已完成）；~~**#1 命令目录（含中文别名与 /export）**~~（已完成）。

**明确不修**（无生产者的风格/抽象差异，理由见修复记录）：#29 事件命名风格、#7 命令作用域分层、#13 层级路径名。

## 修复记录

| 项 | 状态 | 说明 |
|---|---|---|
| #8 cwd | 已修复 | `DelegationOptions.cwd` + 工厂签名改为 `Fn(Option<&str>) -> Result<ChildSession, String>`；新增 `SessionCwd::new_at`（子会话初始目录派生时固定，不写父会话 `WorkingDir` 事件）；三个装配点（assemble / onion-server / onion-sdk-server）均按权限根解析校验 |
| #10 wait_agent | 已修复 | `SubagentHandles.settled` per-agent `Notify`，settle hook 翻转状态并 `notify_waiters`；`wait_agent` 阻塞等待，已 settle 立即返回，父轮次取消时如实报错 |
| #4 sourceEventSeq | 已修复 | `SessionEvent::CommandDone` 新增 `source_event_seq`；`SessionLog::len()` 提供日志下标；`rest/commands.rs` 执行前后取差值定位首条领域事件；`CommandResultInfo` 同步返回；前端 `CommandItem.sourceEventSeq` + `CommandView` 在富卡片承载时隐藏回执行 |
| #14 状态枚举 | 已修复 | `AgentStatus` 收敛为 `Running` / `Inactive`（原 `Settled`），`label()` 下发 `running`/`inactive`，新增 `is_inactive()`；`list_agents` 改为 dsh 渲染形态 `name [status] — description`，首行恒为 root `lead [running] — root agent`；description 文案逐句对齐 dsh「inactive 不代表完成/成功/失败」。dsh 的 `corrupt`/`unsupported`/`unavailable` 属「下游子树不可读」诊断态，onion 扁平活跃表不产生该场景 |
| #18 生命周期事件 | 已修复 | `LiveEvent` 新增 `SubagentStart` / `SubagentEnd`（瞬时、不落盘）；宿主经 `SubagentRuntime::set_parent_log` 注入父会话日志，`create()` 发 start、`drive_subagent()` settle 段发 end，二者以 `run_id` 配对；`stop_reason` 由 `TurnReport.reason` 推导（`completed`/`aborted`/`max-tokens`/`refusal`，异常为 `error`），`last_assistant_message` 取最终 assistant 消息文本；`onion-sdk-client` 同构补齐，chatapp live reducer 对非 `completed` 结束时展示错误 notice |
| #17 子代理描述符 | 已修复 | `SessionEvent::SubagentDescriptor`（durable，写入**子会话**日志，log-only 不进模型上下文）；`SubagentRuntime::create()` 在首个请求前 append 一次，携带 `version`/`mode`/`provider`/`label`/`agent_model`/`agent_reasoning_effort`/`cwd`/`parent_session_id`/`depth`；`SUBAGENT_DESCRIPTOR_VERSION = 1`（格式变更须升版，同 dsh）；冷恢复后可据此枚举子代理元信息。`onion-sdk-client` 同构补齐，TUI 转录与终端 listener 各自忽略/打印 |
| #15 settle 回投 | 已修复 | notice 由「空 blocks 仅唤醒」升级为 dsh 形态的 **model-visible 一行账目**：`settlement_summary()` 逐句对齐 dsh `settlementSummary`（completed / aborted / max-tokens / refusal / error / 未知），并带收尾说明（`closing_delivered` → 「它结束前发过消息，收尾内容已省略」/「它没有收尾消息」，对齐 dsh `omitClosingText`）；`MessageSource::SubagentSettled` 增补 `stop_reason` / `summary` 两个 durable 字段。relay（子代理自己写的内容）与 notice（运行时关于子代理的陈述）仍严格区分 |
| #16 投递策略 | 已修复 | 新增 `subagent::relay::Delivery { Queue, Steer }`（对齐 dsh `SubagentDelivery`）：`Queue → Inbox::followup`（next-turn）、`Steer → Inbox::steer`（next-step）。settle 投递按 `Delivery::for_parent_idle(父会话是否空闲)` 选择，宿主经 `SubagentRuntime::set_parent_idle` 注入探测器（`onion-server` 用 `TurnGate::is_busy()`，settle 时刻求值而非委派时刻）。`send_message` 工具暴露 `delivery` 参数（缺省 `queue`，与 dsh 一致）。投递后仍一律唤醒父会话（dsh `sendWaking` 同语义） |
| #5 / #6 命令名校验与未知名 | 已修复 | `rest/commands.rs` 新增 `valid_name()`（`^[a-z][a-z0-9_-]*$`，对齐 dsh `COMMAND_NAME_RE`）：非法名 → `400 invalid_command_name`，**不落** `command_run`；新增 `known()`：未注册命令只回执 `kind:"error" / "未知命令 /x"`，**不落盘**（对齐 dsh「未知命令不留痕」） |
| #31 未知事件兼容 | 已修复 | `SessionLog::replay_event()`（回放权威路径 `attach`）：**类型未知且未标 `ignorable:true` → 拒绝重建**（`io::ErrorKind::InvalidData`，不再静默跳过）；标了 `ignorable:true` → 跳过；已知类型但字段形状漂移（旧日志）→ 跳过以保兼容。对齐 dsh「未知事件必须显式声明可忽略」 |
| #9 `agent_preset` | 已修复 | 委派工具新增 `agent_preset` 入参（对齐 dsh `preset-selection.ts`）：白名单 = `AgentCatalog::ids()`（内置 coding/work/creator + `agents.yml` 自定义），**仅白名单非空时注入 schema**（`enum` + 继承说明），省略 = 继承父会话预设，白名单为空时显式传值报 `agent_preset selection is disabled for this tool instance`，非法值报 `agent_preset must be one of: ...`。`DelegationOptions` 增 `preset`，宿主工厂签名扩为 `Fn(Option<&str> /*cwd*/, Option<&str> /*preset*/)`，按预设 `tools` 过滤子会话工具面、`system_prompt` 作为 `agent:directive` 段进入子会话提示词（`ChildSession::with_directive`）。对齐 dsh「persona/工具组合一律走预设，不再用 toolFilter/persona 配置」 |
| #11 路由白名单默认值 | 已修复 | 新增 `subagent/model_selection.rs`（对齐 dsh `model-selection.ts` + `model-selection-state.ts`）：durable 事件 `SessionEvent::SubagentModelSelectionPolicy { allowed_models }`，在首个模型请求前由 `seed_policy` 写入（`ONION_SUBAGENT_MODEL_SELECTION` JSON 白名单，缺省不写）；**无该事件时显式选择只允许父会话当前路由**（`assert_allowed_model_selection`，报错附 `without a recorded model-selection policy, explicit selection is limited to the parent route`）；子会话经 `policy_from_events(父日志)` 继承白名单；`list_subagent_models` 的 provider/model 列表同样按白名单过滤 |
| #22 compact 检查点消息 | 已修复 | 新增 `MessageSource::CompactCheckpoint { compaction_id }`（对齐 dsh `source:{kind:'compact-checkpoint'}`）与 `summarizer::frame_checkpoint()`；`run_compaction` 在 `CompactionSummary` 之后紧随 append 一条携带该 source 的 durable `UserMessage`（正文即 `<compacted-summary>` 承载帧），使检查点在日志层显式可识别、且不被当成真实用户输入。投影层替换仍由 `CompactionSummary` 完成（`Context::apply` 对该 source 的 UserMessage 直接跳过，避免双重插入）；`estimate` 不计检查点消息（与 shadowed 区间重叠），goal 权威判定跳过检查点（不污染 `GoalTurnSource`），TUI `injection_kind` 折叠为 `compact-checkpoint` 一行注记 |
| #26 审批 outcome 闭集 | 已修复 | 新增 `onion::approval::ApprovalOutcome`（`allowed-once` / `rejected` / `cancelled` / `unavailable`，对齐 dsh `ApprovalOutcome`）：`ALL` 顺序与 dsh `OUTCOMES` 一致，`as_str()` 只产出闭集字面值，`parse()` 对闭集外输入一律归一为 `unavailable`（fail-closed）。`ApprovalDecision::outcome()` 把回执收敛到闭集（`AllowedForSession` 亦记 `allowed-once`）。`ApprovalAudited.decision` 从此只写闭集值（原 `"allowed"`/`"denied"` 自由串下线）：沙箱/restriction/reviewer/operator 放行 → `allowed-once`，拒绝 → `rejected`，`OperatorReply::Closed`（无审批通道）→ `unavailable`/`fail-closed`，`OperatorReply::Dropped`（撤回）→ `cancelled`/`interrupted`。会话中断 `POST /interrupt` 先调 `SessionChannels::cancel_all("cancelled")` 撤回全部待回填（drop 答复端 → 等待端按 `cancelled` 落审计），请求 id 一并进入幂等窗口；回填晚到时内核等待端已退出 → 409 `request_cancelled` 且幂等记录改记 `cancelled`（`override_decision`），三类回填（审批 / plan 裁决 / 问询）一致 |
| #28 ask_user 迟到回复 | 已修复 | timed 前台等待逾限后工具侧 receiver 已 drop，`reply.send()` 失败即判定「迟到答复」（唯一可靠信号）；三个宿主（`onion-server` question relay / `onion-tui` `/answer` / `onion-cli`）统一改走 `onion::ask_user::answer_to_pending_message()` → `Inbox::steer()` 注入普通 user 消息。payload 对齐 dsh：`{kind:"answer_to_pending_question", tool:"ask_user_question", callId, questions, answers}`，来源标 `MessageSource::AnswerToPending { call_id }`（该变体此前已定义但无生产者）。新增单元测试 `ask_user::service::tests::answer_to_pending_message_carries_call_id_and_questions` |
| #24 compact 回执指针 | 已修复 | `CompactionResult` 新增 `summary_seq`（记 `CompactionSummary` 落盘那一刻的 `SessionLog::len()`，对齐 dsh `result.summarySeq`）；`rest/commands.rs` 的 `Outcome` 增 `source_seq`，`run()` 优先采用命令自带的精确指针、否则退回「首条产物事件下标」；`/compact` 成功路径回 `sourceEventSeq = result.summary_seq`（busy → 信号投递、Nothing → 无指针，行为不变）。新增单元测试 `compaction::listener::tests::compact_now_result_summary_seq_points_at_summary_event`；真实 API 冒烟 `smoke-24-compact-seq.mjs` 校验响应指针与 transcript 中 `compaction_summary` 下标一致（实测 47/47） |
| #1 命令目录 | 已修复 | 服务端新增 `/feedback` 与 `/permission`（对齐 dsh 同名命令）：`SessionEvent::FeedbackRecord`（log-only，正文去首尾空白、空文本省略、可选 `category`）+ `/feedback <text>` 空文本报用法错且**不落盘**、成功回执含会话 id；`/permission [<preset>]` 空参回执当前预设与可选清单、非法预设报 `未知预设「x」（可选：…）`、合法预设走与 `PUT /v1/sessions/{id}/approval` 同源的 `build_approval_config`（`auto` 升级 `LlmReviewer`）。预设表收敛为 `commands::APPROVAL_PRESETS` 单一字面量，`set_approval` 复用避免分叉。**中文别名**：`COMMAND_ALIASES`（目标/计划/反馈/压缩/权限）+ `canonical_name()`，精确名优先、别名次之，别名命中后以**规范名**执行与落盘（时间线不出现别名）；未注册的非 ASCII 拼写（如 `/导出`）按「未知命令」回执而非 400（对齐 dsh `resolveCommand` 返回 undefined 后由执行层报未知）。**`/export`** 落在 chatapp 本地（复用侧边栏 `sessionToMarkdown` + `downloadText`，不接受路径参数，对齐 dsh「Web /export 不接受路径」），无需服务端 ZIP 路由。chatapp 命令面板、help 与 `runSlash` 同步别名归一。新增单元测试 `rest::commands::tests`（5 项）；真实 API 冒烟 `smoke-1-feedback-permission.mjs` + `smoke-1b-command-aliases.mjs`（别名等价、规范名落盘、未注册别名不落盘） |
| #27 委派审批种子 | 已修复 | 新增 log-only durable 事件 `SessionEvent::ApprovalPreset { preset, source? }`（onion 的预设即「沙箱级别 + 审批策略 + 工具限制」组合，故把 dsh 的 `approval/policy` + `sandbox/mode` + `permission/preset` 三事件折叠为一条）。写入口两处：`/permission` 命令与 `PUT /v1/sessions/{id}/approval`，均落 `source: None`（运行期切换）。委派时 `SubagentRuntime::create` 经 `approval::preset_from_events(父日志投影)` 折叠父会话有效预设，以 `source: Some("delegation")`（`DELEGATION_SOURCE`）播种进**子会话自己的**日志；父会话无记录时不写（走装配期默认，同 dsh「未切换的沙箱保持部署默认」）。折叠规则为**最后一条生效**，空预设忽略。新增单元测试 `approval::tests::preset_from_events_takes_last_record`；真实 API 冒烟 `smoke-27-delegation-approval.mjs`：父会话 `/permission danger-full-access` → 委派子代理 → 子日志出现 `{"preset":"danger-full-access","source":"delegation"}`，父日志切换记录无 `source`。附带收益：`/permission` 现在回 `sourceEventSeq` 指向该事件，审批预设可冷恢复重建（此前仅内存态） |
| #25 审批必须 turn 内 | 已修复 | 新增 `approval::turn_open(events)`（最后一条 `TurnStart` 之后无 `TurnEnd` 即开轮）。`ApprovalGate::tool` 在**升级操作**分支（reviewer / 人工复决）前判定：轮次外直接 fail-closed 拒绝，reason 为 `approval requested outside an open turn: the approval audit pair must be turn-enclosed`，且**不落任何审计事件**（对齐 dsh「idle ask rejects before appending anything」）。沙箱允许集直通与 deny 规则不受影响（它们不产生挂起审批）。测试夹具 `gate_ctx` 补 `TurnStart` 以模拟真实工具调用；新增 `approval::tests::turn_open_tracks_last_turn_boundary` 与 `gate_rejects_approval_outside_open_turn`；真实 API 冒烟 `smoke-25-approval-in-turn.mjs` 校验写工具审批审计全部落在 `turn_start`/`turn_end` 之间且工具执行成功 |
| #12 路由预检 | 已修复 | 新增 `tools::subagent::preflight_route(&ModelTarget)`（对齐 dsh `preflightChildLlmRoute`）：对**显式选择**的路由在创建子会话前发一次真实 `POST {base_url}/chat/completions`（`max_tokens: 1`，10s 超时），非 2xx 或连接失败即返回 `child LLM route "<model>" preflight failed: ...`（上游报文截断 300 字符）。`subagent` 与 `subagent_fork` 两个工具在 `parse_delegation` 之后、`activate` 之前调用；继承父路由时跳过（父会话已在用该路由，重复探测纯属浪费）。新增单元测试 `preflight_route_rejects_unreachable_base_url`（真实网络调用，保留端口 1 必然拒连）；真实 API 冒烟 `smoke-12-route-preflight.mjs`：显式 `provider=local/model=Qwen3.8-27B` 委派成功，子会话描述符 `agent_model` 为该模型 |
| #30 surface 白名单 | 已修复 | 新增 `SessionEvent::is_surface()`（对齐 dsh `SurfaceEventType` 白名单）：只有 `SystemMessage` / `UserMessage` / `AssistantMessage` / `ToolResult` 四类为 surface 事件，其余（含 `CompactionSummary`、`CommandRun/Done`、`ApprovalAudited`、`FeedbackRecord`、`ApprovalPreset` 等 log-only 事件）一律不是。onion 不引入 `surfaceOp` 字段（投影由 `Context::apply` 按类型分支完成，`CompactionSummary` 直接改写区间，等价 dsh 的 `surfaceOp: replace`），本方法把「哪些事件进入模型可见上下文」显式化为单一判据。新增单元测试 `session::tests::surface_whitelist_matches_dsh`（4 类正例 + 6 类反例） |
| #29 事件命名风格 | 不修（决策） | dsh 用域前缀 + `/`（`command/run`），onion 全 snake_case 单段（`command_run`）。这是**纯 wire 命名风格**，不影响任何语义：onion 的 durable 日志、REST DTO、前端 reducer 已全链路一致，改名需同步 `SessionEvent` 全部变体 + 40+ 处 serde 标签 + 前端 `frame-reducer` + 既有 transcript 兼容（旧日志会因类型未知被 #31 的严格回放拒绝）。收益为零、风险与改动量最大，故明确**不修** |
| #7 命令作用域分层 | 不修（决策） | dsh 的 `ScopedLayers` 服务于「插件可注册 agent 私有命令」；onion 无插件注册面，命令是宿主内置的 5 个固定能力，不存在作用域覆盖的生产者。引入分层只会增加无消费者的抽象 |
| #13 层级路径名 | 不修（决策） | dsh 全名是 `/` 路径（`lead/worker`），因为其子代理可递归派生。onion 子代理深度恒为 1（子会话工具面不含 subagent 工具，天然不递归），全名 ≡ 单段名，层级路径无生产者；字符集/长度/`lead` 保留/唯一性已对齐 |
| 其余 | 无待办 | 32 项差异已全部处置：已修复 24 项，明确不修 3 项（#29 / #7 / #13，均为无生产者的风格或抽象差异），其余为「已对齐」项 |

### 冒烟验证（2026-11-14，真实 API local/Qwen3.8-27B）

委派 `cwd:"docs"` + `wait_agent` 的完整链路：

```
list_subagent_models → local/Qwen3.8-27B
subagent {name:"docs-scout", description:"统计文档数量", cwd:"docs", provider:"local", model:"Qwen3.8-27B"}
  → "started subagent docs-scout (onion-1794624009934)"
wait_agent {agent:"docs-scout"} → "docs-scout settled"        ← 阻塞至 settle，未轮询
wait_agent {agent:"docs-scout"} → "docs-scout already settled" ← 幂等立即返回
```

子代理侧证据：`glob {pattern:"*.md"}`（相对当前目录）直接命中 `docs/` 下文件，报告「当前目录（含子目录）下共有 29 个 .md 文件」——修复前需写 `ava2/docs` 全路径且解析失败。

效果对比：修复前 7 步（含 3 次 `list_agents` 轮询）；修复后 4 步（1 次 `wait_agent` 到位）。

#### #17 描述符落盘验证（session `onion-1794647408445` → 子 `onion-1794647418927`）

子会话日志 `<ONION_HOME>/logs/subagents/onion-1794647418927/session.v1.jsonl` 事件序（描述符是**第一条**，早于 `session_started`，即首个请求之前）：

```
subagent_descriptor
session_started → turn_start → step_start → system_message → user_message ×2
→ assistant_message → step_end → turn_end
```

```json
{"event":{"agent_model":"Qwen3.8-27B","depth":1,"label":"描述符冒烟","mode":"one-shot",
          "parent_session_id":"onion-1794647408445","provider":"session",
          "type":"subagent_descriptor","version":1},"ts":1794647418928}
```

`cargo test -p onion subagent` → 15 passed / 0 failed。（`shell_exec::oneshot_tests::spawn_plan_pwsh_uses_encoded_command` 为既有 Windows 编码相关失败，与本改动无关。）

#### #15 settle 通知验证（父 `onion-1794648429549` → 子 `onion-1794648439439`）

父会话 durable 日志中两条投递（source 严格区分）：

```json
{"agent_id":"onion-1794648439439","kind":"subagent_message"}          ← relay（子代理自己写的内容）
{"agent_id":"onion-1794648439439","kind":"subagent_settled",
 "stop_reason":"completed",
 "summary":"Background subagent desc-child finished and will do no further work unless you send it more."}
```

notice 的 model-visible 文本（两段）：

```
Background subagent desc-child finished and will do no further work unless you send it more.
It sent you a message just before finishing, so its closing message is omitted.
```

`cargo test -p onion subagent` → 16 passed / 0 failed（新增 `notice_carries_summary_and_stop_reason`、`notice_reports_failure_reason_for_parent`）。

#### #16 投递分区验证（父 `onion-1794651547216`，父轮次先结束、子代理后 settle）

```text
[turn] t1 · 2 steps · completed          ← 父代理委派后立刻收轮，进入 Idle
[subagent] end idle-child stop=completed
[inbox] inserted next-turn "1… 2… 3\ndone"          ← relay：父 idle → queue（不是 next-step）
[inbox] inserted next-turn "Background subagent idle-child finished …"  ← notice 同样 queue
[state] Idle -> Running(Claiming)
[inbox] claimed next-turn x1
turn_start t2 → 消费 relay
turn_start t3 → 消费 notice
```

对照：父代理仍在运行（`wait_agent` 场景）时，同一路径打印 `inserted next-step`，由当前 turn 的下一步消费——与 dsh `parent.status === 'idle' ? 'queue' : 'steer'` 一致。

已知残留差异：onion 的 `claim_open()` 每轮只领取**一条** next-turn，故 relay 与 notice 被拆成 t2/t3 两个 turn 消费；dsh 单次 wake 会把同批队列消息合并进一个 turn。该差异属 kernel claim 语义，不在 #16 范围。

### #4 冒烟验证（REST + 浏览器）

```
POST /v1/sessions/{id}/commands {"name":"goal","args":"完成交互差异文档的修复"}
  → {"commandId":"cmd-…-goal","kind":"success","sourceEventSeq":1}     ← 有领域事件
POST … {"name":"compact"}  → 无 sourceEventSeq 字段                      ← 无产物（Nothing）
POST … {"name":"bogus"}    → 无 sourceEventSeq 字段                      ← 未知命令
```

durable 落盘：`{"type":"command_done","command_id":"cmd-…-goal","kind":"success","source_event_seq":16,…}`

前端渲染（截图 `ava2-source-event-seq-goal.png`）：`/goal` 仅剩回显气泡，回执行消失，目标由底部 GoalBar 承载——对齐 dsh「handler 结果不回声，由持久 flow node 渲染」。

> 注意：chatapp 是**独立 esbuild 产物**（`apps/chatapp/dist/main.js`，经 `/v1/apps/` 由边车分发），改前端源码后必须 `npm run build` 并拷贝到 `<ONION_HOME>/apps/chatapp/`，Vite 热重载不覆盖该产物。

### #14 / #18 冒烟验证（真实 API local/Qwen3.8-27B，session `onion-1794635543885`）

SSE `/v1/sessions/{id}/events` 抓到的 live 变体：`InboxInserted` / `StateChanged` / `InboxClaimed` / `AssistantStream` / `SubagentStart` / `SubagentEnd`。

```
SubagentStart {run_id:"onion-1794635558738-1794635558739", agent_id:"onion-1794635558738",
               name:"smoke-child", depth:1, parent_session_id:"onion-1794635543885"}
SubagentEnd   {同上 run_id, stop_reason:"completed",
               last_assistant_message:"\n\n1+1 等于 2。"}
RESULT paired: true  start: 1  end: 1          ← start/end 以 run_id 严格配对
```

`list_agents` 工具返回（父代理透传原文）：

```
lead [running] — root agent
smoke-child [inactive] — 冒烟子代理 1+1
```

即：root 行前缀 `lead [running]`，子代理 settle 后翻转为 `inactive`（无 turn 在执行），description 紧跟其后——与 dsh `render` 形态一致。

前端复核（chatapp @ `http://localhost:1420/`，会话 `onion-1794635543885`）：工具步骤折叠分组显示 `已完成 · 4 步`（两次 `subagent`、`wait_agent`、`list_agents`，首次因名字含下划线被拒并标红），`list_agents` 结果为代码块：

```
lead [running] — root agent
smoke-child [inactive] — 冒烟子代理 1+1
```

配套修复：裸边车形态（不经 Tauri 壳单独起 `onion-server`）此前未注入 `ONION_SERVER_CORS_ORIGINS`，浏览器跨源访问 `/v1/*` 全部 `Failed to fetch`，导致 AppHost 读不到 App 列表、chatapp 无法加载。已在 `dev.ps1` 的环境变量段补上该变量（与 `src-tauri/src/sidecar.rs` 的 `CORS_ORIGINS` 保持一致）；重启后 `OPTIONS /v1/apps` 返回 200 且带 `access-control-allow-origin: http://localhost:1420`。
