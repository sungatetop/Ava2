---
title: chatapp 三控件方案：工作区切换 / 模型切换 / 审批模式切换
description: 对话 App 头部补齐工作区、模型、审批模式三个会话控制控件：后端 REST 端点扩展（onion-server）+ AgentChannel 契约扩展 + chatapp UI 交互设计。
keywords: chatapp, 工作区切换, 模型切换, 审批模式, ModelTargetRef, ApprovalConfig, REST, AgentChannel
author: AI 助手
date: 2026-11-09
updated: 2026-11-09
status: done
category: solutions
---

# chatapp 三控件方案：工作区切换 / 模型切换 / 审批模式切换

## 1. 背景与目标

chatapp 头部目前只有会话切换与新会话，缺三类会话控制：

1. **工作区切换**：新建会话时指定 cwd（`createSession(cwd?)` 已支持，但无 UI）；
2. **模型切换**：会话运行期换模型；
3. **审批模式切换**：会话运行期换审批预设。

## 2. 引擎能力面摸底结论

| 能力 | 现状 | 结论 |
| --- | --- | --- |
| 模型 | 引擎已有 `ModelTargetRef`（`current`/`assembled` 双 ArcSwap）+ `Runtime::set_model_target_sync`；Kernel 在 prompt-assembly 边界快照并自动注入 modelSwitchNotice。**但 onion-server 的 `assemble_session` 未把共享 target 绑到 `OpenAiSseLlm`（`with_target` 缺失）**——kernel 换 model 生效、base_url/api_key 不随 provider 路由 | 补齐共享绑定即可全链路热切换（对齐 onion SDK 侧 assemble.rs 既有做法）；`ProviderCatalog::target_for(model)` 已能按模型 id 解析 (base_url, api_key, effort) |
| 审批 | 会话内 `approval_config: Arc<RwLock<ApprovalConfig>>` 由 ApprovalGate 持有，**运行期可写**；`build_approval_config(preset, llm, model)` 已存在（auto 预设升级 LlmReviewer） | 把该 Arc 存入 SessionHandle，REST 写入即生效；预设集合 = `for_preset` 精确分支：`workspace-write` / `auto` / `danger-full-access`（`custom`/未知名回退 workspace-write，不对外提供） |
| cwd | 权限根装配期固定，`SessionCwd::set` 仅可在根内移动（模型已可经 working_directory 工具做）；跨工作区 = 新会话（`createSession(cwd)` 已支持，root=该 cwd） | UI 走「选择工作区 → 新建会话」路径，不给既有会话加跨根改 cwd 端点 |

## 3. 后端设计（onion-server）

### 3.1 装配层（session.rs）

- `assemble_session`：构造共享 `model_target = Arc<ModelTargetRef>`（初始三元组来自 config），`llm.with_target(model_target.clone())`；`Runtime::resume` 后 `runtime.set_model_target_sync(model_target.clone())`——kernel 与 llm 共享同一引用，`swap_current` 双侧生效。
- `SessionHandle` 新增三字段：`model_target` / `approval_config` / `llm`（审批预设切到 `auto` 时重建 LlmReviewer 需要）。

### 3.2 REST 端点（control.rs）

| 端点 | 语义 |
| --- | --- |
| `PUT /v1/sessions/{id}/model` body `{model}` | `ProviderCatalog::target_for` 解析路由；目录无此模型 → 400 `unknown_model`；`swap_current` + 追加 durable 审计事件 `ModelSelection`；返回刷新后的 SessionInfo |
| `PUT /v1/sessions/{id}/approval` body `{preset}` | 预设白名单（三值）；`*approval_config.write() = build_approval_config(...)`；返回 SessionInfo |

- `SessionInfo` 增 `model`（`model_target.current().model`，即下一请求目标）与 `approvalPreset`（`approval_config.read().preset.name`）——读面无需独立 GET 端点。
- 运行中切换语义：不中断当前 step，下一请求生效（引擎既有 dsh 对齐行为）；审批预设写入后下一工具调用即生效。
- 持久化边界：模型切换审计落 durable 事件（回放可见），但会话冷恢复仍回到装配期默认模型（target 不随日志重建）——与 settings.json 用户级写回（SDK 侧方案 4）区分，v1 不做。

## 4. 契约扩展（host）

- `types.ts`：`SessionInfo` 增 `model` / `approvalPreset`；`AgentChannel` 增 `setSessionModel(id, model)` / `setApprovalPreset(id, preset)`（均返回 SessionInfo）。
- `agent.ts`：对应 PUT 封装。

## 5. chatapp UI 设计

头部控件排布（现有 `cc-head` 行内追加，均限域 `.cc-root`）：

1. **工作区**：显示当前会话 `workingDir`（title 悬浮全路径）；「切工作区」按钮 → `window.prompt` 输入绝对路径 → 校验非空 → `createSession(cwd)` + attach + 写入 storage 近用列表；下拉记录最近 5 个工作区，选中即按该 cwd 新建会话。
2. **模型**：`listModels()` 展平 providers → `<select>`；值为当前会话 `sessionInfo.model`；onChange → `setSessionModel` → 用返回的 SessionInfo 刷新（失败回滚 + 错误条）。
3. **审批模式**：三值 `<select>`（workspace-write=工作区写直通 / auto=自动审批 / danger-full-access=全部放行）；值来自 `sessionInfo.approvalPreset`；onChange → `setApprovalPreset`。

状态：`sessionInfo` 由 attach/getSession/两处 PUT 统一维护；近用工作区与上次模型选择走 `ctx.storage`（`recentWorkspaces` / `lastModel`），新建会话时若 storage 有 `lastModel` 则创建后立即应用（保证「下次新会话还用我选的模型」）。

## 6. 验收

- cargo test -p onion-server（新增模型/审批端点测试）；
- CDP 冒烟：切审批模式 → REST 读回一致；切模型 → 真实对话走新模型（resp 上报模型名/审计事件）；切工作区 → 新会话 workingDir 生效。
