---
title: ava2 onion 对齐 dsh 实施方案（独立边车架构）
description: 以 deepseek-harness 为基准，针对 ava2「onion 内核 crate + onion-server 边车 + chatapp 前端」三层架构，给出 subagent 委派、斜杠命令、plan/goal/compact、审批与 ask_user、会话事件模型的逐项落地步骤与验收口径。
keywords: onion, dsh, deepseek-harness, 对齐方案, 边车, onion-server, subagent, 斜杠命令, 事件模型
author: AI 助手
date: 2026-11-14
status: review
category: solutions
---

# ava2 onion 对齐 dsh 实施方案（独立边车架构）

## 1. 架构前提（决定方案的三个约束）

ava2 与 ava_square 的根本差异在于 onion 的宿主形态，这决定了同一项 dsh 能力在两边的落点完全不同。

| 维度 | ava2（本方案） |
| --- | --- |
| onion 形态 | 独立 crate，经 `onion-server` 边车进程承载 |
| 装配位置 | `crates/onion-server/src/session.rs::assemble_session` |
| 驱动位置 | `crates/onion-server/src/session.rs::spawn_turn`（TurnGate 保证 driver 唯一） |
| 事件出口 | `SessionNotice` → SSE `/v1/sessions/{id}/events` + WS |
| 前端 | `apps/chatapp`（独立 esbuild 产物，改动需重新 build 并拷贝到 `<ONION_HOME>/apps/chatapp/`） |
| 会话粒度 | 一个 Runtime 一个 session；边车进程内多条会话共存 |

由此产生三条 ava2 专属约束：

1. **跨层契约优先**：内核 `SessionEvent` / `LiveEvent` 的新增必须同时打通 `SessionNotice` 映射与 chatapp 渲染，否则前端不可见。
2. **唤醒回调不能裸闭包持有 `Arc<SessionHandle>`**：`SubagentRuntime` 与 `SessionHandle` 互相引用会泄漏，必须用 `Weak`。
3. **命令层在服务端**：`rest/commands.rs` 是唯一命令执行面，前端只做本地命令分流与事件消费。

## 2. 分层落点矩阵

每一项差异标注它在哪一层落地，避免把宿主职责塞进内核。

| 层 | 职责 | 代表文件 |
| --- | --- | --- |
| L1 内核契约 | 事件定义、Runtime 钩子、工具 schema、投影 | `crates/onion/session.rs`、`subagent/*`、`tools/subagent.rs` |
| L2 边车宿主 | 装配、驱动、REST/SSE、命令执行 | `crates/onion-server/src/{session.rs,rest/*}` |
| L3 前端 | 命令分流、事件消费、富卡片渲染 | `apps/chatapp/src/*` |

## 3. P0：运行时正确性问题

### 3.1 父会话空闲时子代理回投不被消费（ava2 现存缺口）

apl 现状：relay 走 `Inbox::steer`（next-step + 唤醒标志），但 `next-step` 只在 `spawn_turn` 的循环内被消费；父会话 idle 时 driver 已退出，消息滞留到下一次用户输入才被领取。dsh 用 `manager` 的唤醒回调即时驱动。

落地方案（保持边车形态）：

1. L1：`crates/onion/subagent/runtime.rs` 增加
   ```rust
   parent_wake: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
   pub fn set_parent_wake(&self, wake: Arc<dyn Fn() + Send + Sync>),
   fn parent_wake(&self) -> Option<Arc<dyn Fn() + Send + Sync>>,
   ```
   并在 `drive_subagent` settle 收尾（relay + notice 之后）调用 `wake()`。
2. L2：`assemble_session` 装配完成后注入，用 `Weak` 断开循环引用：
   ```rust
   let weak: Weak<SessionHandle> = Arc::downgrade(&handle);
   subagent_runtime.set_parent_wake(Arc::new(move || {
       if let Some(h) = weak.upgrade() {
           crate::session::spawn_turn(&h);
       }
   }));
   ```
   注意装配时序：`SessionHandle` 构造完成（`runtime.inbox()` 可用）之后才能注入。
3. 幂等保护：`spawn_turn` 已有 `gate.try_acquire_driver()`，重复唤起安全；`drive_subagent` 异常路径也必须触发 wake，否则父会话永久挂起。

验收：父会话 idle 时启动子代理，子代理 settle 后**无需用户输入**父会话即开新一轮并消费 relay。

### 3.2 `wait_agent` / `cwd` / `name` 已完成项的回归锁定

已落地（见 `onion-dsh-interaction-gaps.md` 修复记录）。本方案只需补测：
- 边车重启后 `settled` 表为内存态，`wait_agent` 对已消失的 id 必须返回明确错误而非挂起；
- `cwd` 越界（`../` 逃逸权限根）必须拒绝，覆盖 `assemble` / `onion-server` / `onion-sdk-server` 三个装配点。

## 4. P1：能力补齐

### 4.1 斜杠命令层（L2 + L3）

| 步骤 | 内容 | 文件 |
| --- | --- | --- |
| 1 | 命令名正则 `^[a-z][a-z0-9_-]*$` 校验，非法名直接 400 不落盘 | `crates/onion-server/src/rest/commands.rs` |
| 2 | 未知命令不落 `CommandRun`/`CommandDone`（当前会落盘 `/未知命令`） | 同上 |
| 3 | 命令别名表（中英别名，对齐 dsh `builtin.ts`） | 同上，常量表 |
| 4 | 补齐 `feedback` / `permission` / `export` 三个命令（`export` 走 deliverables 导出，`permission` 复用 `control::set_approval`） | 同上 + `rest/control.rs` |
| 5 | chatapp 本地命令（help/new/stop/model/approval）迁移或明确标记为「客户端专用」，与服务端命令表合并成一张前端可见目录 | `apps/chatapp/src/chat-view.tsx` |

### 4.2 subagent 生命周期可见性（L1 → L2 → L3）

1. L1 `crates/onion/session.rs` 新增 durable `SubagentDescriptor { id, name, parent_id, provider, model, reasoning_effort, preset, persona, tool_filter }`；spawn/fork 时 append。冷恢复后 `list_agents` 可从日志复原。
2. L1 `LiveEvent` 新增 `SubagentStarted { id, name }` / `SubagentEnded { id, name, stop_reason }`（live-only，不落盘）。
3. L2 无需改动：`SessionNotice::Live { event }` 自动透传；仅 SSE `event_name()` 不带分支变化。
4. L3 chatapp 消费 live 事件渲染子代理卡片，替代 `list_agents` 轮询。

### 4.3 relay 语义补全（L1）

`crates/onion/subagent/relay.rs` 的 notice 与 relay 两条消息体补 `stop_reason` 与 `summary`；`types.rs` 的 `AgentStatus` 扩为 `Running | Inactive | Corrupt | Unsupported | Unavailable`（保留 `Settled` 兼容旧日志反序列化为 `Inactive`）。

### 4.4 路由政策持久化（L1 + L2）

- L1：新增 durable `SubagentModelSelectionPolicy { parent_route, routes }`，子会话继承父 policy；
- L2：`resolve_route` 由「目录即白名单」改为「policy ⊆ 目录」（无 policy 时只允许沿用父路由）；
- 预检：借用 `onion::provider_catalog` 做静态校验即可，真实连通性预检放到 `POST /v1/models/test`（边车已有），不在委派路径上做网络调用。

## 5. P2：一致性打磨

| 项 | 落点 | 说明 |
| --- | --- | --- |
| `compact-checkpoint` 消息 | L1 `compaction/listener.rs` | 压缩后追加 source 为 `compact-checkpoint` 的 user 消息，投影据此替换而非依赖 `CompactionSummary` 区间 |
| compact 回执指针 | L2 `rest/commands.rs` | 已有 `source_event_seq` 机制，`compact` 分支返回 `summarySeq` 即可 |
| 审批 outcome 闭集 | L2 `session.rs` | `ApprovalDecision` 增加 `Cancelled` / `Unavailable`；会话关闭时把 pending 审批批量置 `Cancelled` 并推 SSE |
| 审批 delegation 来源 | L1 + L2 | 子代理审批请求携带 `source: "delegation"` 字段，前端展示来源 |
| ask_user 迟到回投 | L2 `rest/inputs.rs` | 超时后保留 requestId 一段时间，迟到答复以 `user-question-reply` 事件回投并返回 409 之外的明确语义 |
| `ignorable` 兼容 | L1 `session.rs` 反序列化 | 未知事件类型若带 `ignorable:true` 跳过，否则拒绝重建 |
| surfaceOp 白名单 | L1 + L3 | 仅 5 类 surface 事件允许带 `surface_op`，前端按白名单渲染 |

## 6. 交付顺序与门禁

1. **阶段一（运行时正确性）**：3.1 唤醒回调 → 3.2 回归测试。门禁：真实 API 冒烟（local/Qwen3.8-27B）验证父会话 idle 自唤起。
2. **阶段二（命令层）**：4.1 → 4.3。门禁：`/compact` `/goal` 走 REST 返回 `sourceEventSeq`；chatapp 富卡片不回执行。
3. **阶段三（生命周期与政策）**：4.2 → 4.4。门禁：SSE 可见 `subagent_started/ended`；policy 越界被拒。
4. **阶段四（打磨）**：第 5 节逐项，可并行拆分。

## 7. 与 ava_square 方案的关系

ava_square 为进程内嵌装配，无 `SessionNotice`/SSE 抽象，其唤醒链路已存在（`parent_wake` → `drive::spawn_turn`），但缺 `DelegationOptions` / `wait_agent` / 命令事件层。两仓库**不做逐行同步**：内核契约（L1）保持一致，宿主适配层（L2/L3）各自实现。

反向需要回补到 ava2 的 ava_square 存量优势：
- `parent_wake`（本文 3.1 已覆盖）
- `SessionEvent::PermissionPreset`（审批预设变更落盘）
- `Inbox::promote`（排队消息提升为 next-step）
