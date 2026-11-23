---
title: chatapp 斜杠命令与 goal/plan/todos 展示方案（对齐 dsh）
description: 对话 App 补齐斜杠命令面板（8 命令，含 /goal、/plan）与会话目标横幅、plan 工具条徽标、待办面板展示：AgentChannel 契约扩展 + chatapp UI 设计 + 冒烟结论，呈现方式严格对齐 deepseek harness（todos 不是斜杠命令）。
keywords: chatapp, 斜杠命令, goal, plan, todos, TodoPanel, GoalBar, PlanModeControl, dsh
author: AI 助手
date: 2026-11-11
updated: 2026-11-11
status: done
category: solutions
---

# chatapp 斜杠命令与 goal/plan/todos 展示方案（对齐 dsh）

## 1. 背景与目标

chatapp 已有会话/模型/审批/工作区控件（见 `chatapp-controls.md`），但缺三类会话级能力的前端入口：

1. **斜杠命令系统**：dsh CLI/TUI 的 `/help` `/new` `/stop` `/compact` `/model` `/approval` `/goal` `/plan` 在 web UI 无对应操作面；
2. **goal（会话目标）**：onion 已有 `GET/PUT /v1/sessions/{id}/goal` 与 `POST /goal/actions`，前端无展示与操作；
3. **plan 模式 / todos**：durable `plan_mode` / `plan_wanted` / `todo_write` 事件已落盘，前端无消费。

目标：斜杠命令经**本地执行**（REST/SDK 直调 + notice 回执条目）达成；goal / plan / todos 的呈现**严格对齐 dsh**——三者都不是斜杠命令的产物视图，而是 composer 周边与轮次内的原生 UI。

## 2. 契约扩展（host SDK）

`src/host/types.ts` 新增类型与 `AgentChannel` 六方法（`src/host/agent.ts` 对应 REST 封装）：

| 方法 | 端点 | 语义 |
| --- | --- | --- |
| `getTodos(id)` | `GET /v1/sessions/{id}/todos` | 待办快照（末次 todo_write 回放），`{todos} ?? []` |
| `getGoal(id)` | `GET /v1/sessions/{id}/goal` | `GoalInfo \| null`（404 → null） |
| `setGoal(id, input)` | `PUT /goal` | 创建（无 revision）/ 编辑（已存在必须带 revision，否则 409） |
| `goalAction(id, action, revision, reason?)` | `POST /goal/actions` | `pause/resume/complete/block/clear`，clear 返回 null |
| `getPlan(id)` | `GET /v1/sessions/{id}/plan` | `PlanInfo {active, pending}` |
| `setPlan(id, active)` | `POST /plan` | busy 时 `pending=true`（延迟到步骤边界生效） |

关键类型：`GoalInfo {id, revision, objective, phase, blockedReason?, maxGoalRounds, roundsStarted, createdAtMs, updatedAtMs, armed}`（phase ∈ active/paused/blocked/complete）；`TodoItem {content, status: pending|in_progress|completed}`。

durable 事件消费（chatapp `handleFrame`）：

- `todo_write` → `setTodos(todosFromEvent(event))`（**只喂待办面板**，不再生成消息流卡片）；
- `goal_change` → `goalFromEvent` 归一后立即渲染，随后以 `GET /goal` 回填——**durable 快照不含 `armed`**（激活是进程内状态、不落盘），不回填会把新建目标误显示成「待继续的目标」；
- `plan_mode` → `plan`；`plan_wanted` → `pending`。

## 3. chatapp UI 设计

### 3.1 斜杠命令

- 命令清单 `SLASH_COMMANDS` 只列**没有等价操作面的 3 条**：`/compact`、`/goal`、`/plan`。新建会话、中断轮次、切换模型、切换审批预设都有常驻控件（侧边栏新建、发送键停止态、模型 chip、审批 chip），不再重复进菜单；**没有 `/todos`**——dsh 无此命令，todos 不通过命令查看。
- `/help`、`/new`、`/stop`、`/model`、`/approval` 的实现仍保留（完整命令名直接回车仍可执行），但**不出现在菜单里**；`/help` 只列出上述 3 条。其中 `/new` 已是草稿语义：只清空视图，会话在首条消息发送时才创建（见 [chatapp-lazy-session.md](./chatapp-lazy-session.md)）。
- 菜单仅在输入匹配 `/^\/(\S*)$/`（无空格参数）且非 busy 时出现，按命令名前缀过滤，↑↓ 循环、Tab/Enter 选中、Escape 暂时收起（`slashDismissed`）。
- **选中规则（关键）**：带参命令 Tab 补全为 `/name `；**Enter 仅在输入恰为完整命令名时直接执行**（支持 `/goal`、`/plan` 的无参查看），否则填充参数占位——否则无参查看会被菜单拦截成填充，永远无法执行（冒烟中发现的真实缺陷）。
- 执行路径 `runSlash`：全部本地执行（SDK 直调），结果以 `notice`（info/error）条目进消息流，不向 agent 投递、不占 busy；`/stop` 等在生成中可用（斜杠拦截在 busy 检查之前）。未知命令（含已移除的 `/todos`）回执「未知命令 /xxx」。
- `/goal` 子命令：无参查看；`pause/resume/complete/block/clear` 走 `goalAction`（带当前 revision）；其余文本 → 创建/编辑目标。`/plan on|off` 切换、无参查看；pending 时回执注明「（当前轮次结束后生效）」。回执的开/关态取**请求目标**而非服务端返回快照（busy 中切换时快照仍是当前态，按快照判断会把「已关闭」说成「已开启」）；`setPlan` 返回值本地同步到 `plan` 状态——busy on → chip 立即以 `is-pending` 态出现、busy off → chip 立即隐藏，无需等 durable 事件回放。另两条实测语义：草稿态（无会话）执行 `/plan` 回执「无会话」错误；plan 模式开启时模型不执行工具（计划语义），busy-pending 用例须在 plan 关闭状态下用长工具轮次验证。
- 菜单 overlay 绝对定位于 `.cc-composer` 内部（`position: relative`）。

### 3.2 状态展示区（对齐 dsh 插槽语义）

todos 在 dsh 里只有**两个呈现面**，本方案照此收敛（此前的 header chip + 消息流卡片 + `/todos` 命令已全部拆除）：

| 位置 | 组件 | dsh 对应 | 规格 |
| --- | --- | --- | --- |
| composer 上方 dock（order 0） | `TodoPanel` | `conversation.input.dock` TodoPanel | 空清单 `return null` 不占位；**默认折叠**；折叠头 = checklist 图标 + 「任务」+ 进度文本 + chevron（折叠 ↑ / 展开 ↓）；进度只列非零段（`n 已完成` / `n 进行中` / `n 待处理`），en space + `·` 分隔；展开为 `ul`（gap 8、max-height 180、可滚动），每项状态点 + 13/20 单行省略文本 |
| composer 上方 dock（order 10） | `GoalBar` | `ui-goal` GoalBar | **phase=complete 或目标不存在时完全不渲染**；36px 高单行卡片；`目标图标(14) + phase 标签 + objective(省略) + 图标动作组`；phase 文案带「目标」后缀（进行中的目标 / 待继续的目标 / 已暂停的目标 / 受阻的目标）；blocked 时 title 显示 `blockedReason.message`；动作为 28×28 圆形 iconBtn：active+armed→pause、否则→resume，另有 edit、clear |
| composer 底部工具条 | `PlanChip` | `conversation.input.plan`（PlanModeControl） | **不在 header**；有效目标为 plan 模式时才渲染（`effective = pending ? !active : active`）；点击 = 执行 `/plan off`；图标 rest 为 plan、hover 换 closeCircle（暗示可退出）；文案「计划模式」 |
| 轮次过程区内 | `todo_write` ToolRow | `ui-tool` todo-row | 标题「更新任务清单」；摘要 `{done}/{total} 已完成`，有 in_progress 时追加 ` · {首个进行项内容}`（截断 48 字），并行进行项余数为独立 `+N` 后缀 |

状态点 `TodoStateDot`（对齐 dsh `StateDot`）：`completed` = 6px 实心点（success）、`in_progress` = 14px 旋转环（全环 25% 透明度 track + 1/4 弧 dasharray + 0.9s 线性无限旋转）、`pending` = 6px 中性点（t3 / 0.45 透明度）。

`export.ts` / `trajectory.tsx` 对 notice 条目按 `kind !== "turn"` 跳过（不进导出与轨迹）。

### 3.3 发送键的运行态

- **空闲**：圆形蓝色发送键（上箭头），输入为空时禁用（opacity 0.4）。
- **生成中（busy）**：同一个按钮切成**停止键**——方形实心图标 + 深色实心底（`--cc-t1`，图标取反色），`title` / `aria-label` 变为「停止生成」，点击调 `agent.interrupt(id)`；停止键**不受输入为空影响**（始终可点）。
- 中断只是请求：`busy` 由 `turn_report` 收束后解除（不提前乐观解除），只有请求本身失败才立刻回退 busy 并显示错误；`/stop` 与按钮同源。

## 4. 验收

- `npx tsc --noEmit`：chatapp 与 ava2 根均 0 错。
- playwright 冒烟（`ava2-verify/dsh-todo-goal-plan-smoke.mjs`，onion-server 9800 无 token + CORS `http://localhost:1420`，真实模型轮次写 todo）**14 项全 PASS**：
  1. 斜杠菜单恰为 3 条（`/compact`、`/goal`、`/plan`）；2. `/todos` → 「未知命令」；3. `/help` 只列 3 条且不含 `/todos`；4. `/new` 后无 goal/todos 残留；
  5. `/goal` 创建 → GoalBar 在 composer dock、label=进行中的目标、3 个动作；6. GoalBar pause → 已暂停的目标、resume → 进行中的目标；
  7. `/plan on` → 徽标在 `.cc-bar-left`（header 内 0 个）、点击后消失；8. 真实轮次 → TodoPanel 默认折叠、进度「1 进行中 · 1 待处理」、展开 2 行 + 1 个旋转环 + 1 个静态点、`todo_write` 工具行摘要 `0/2 已完成 · 验证待办面板`；
  9. `/goal clear` 后 GoalBar 不渲染；10. 亮暗双态截图。
- 截图：`.screenshots/ava2-host/06-slash-menu.png`、`06-goalbar.png`、`06-planchip.png`、`06-todopanel-collapsed.png`、`06-todopanel-expanded.png`、`06-toolrow-summary.png`、`06-final-light.png`、`06-final-dark.png`。
- 发送键状态冒烟（`ava2-verify/stop-button-smoke.mjs`）3/3 PASS：空闲态为发送（禁用）、生成中为停止（方形图标可点）、点击后 1s 内回到发送态且无错误；截图 `07-stop-button.png`、`07-stopped.png`。
- `/plan` 专项冒烟（`ava2-verify/plan-command-smoke.mjs`）**9 项全 PASS**：菜单含 /plan（恰 3 条）、草稿态守卫、无参查看 off/on、`/plan on` → chip 在工具条（header 0 个）、idle off → chip 消失、**busy 中 on → pending**（回执带「生效」+ chip `is-pending`）、轮次边界后保持 on、非法参数提示；截图 `09-plan-chip.png`、`09-plan-pending.png`、`09-plan-final.png`。

## 5. 已知边界

- 目标编辑：GoalBar 的 edit 动作把 `"/goal " + objective` 回填到 composer（走 `/goal` 更新链路，带当前 revision）；并发修改（rev 过期）由后端 409 兜底。
- `armed` 只能从 REST 读面获得：durable `goal_change` 快照不含该字段，故每次 goal 事件后追加一次 `GET /goal` 回填（一次轻量请求，换取 phase 文案准确）。
- `/compact` 仅 1 参（SDK `compact(id)`），不支持自定义说明。
- 待办无可选清空命令（引擎无对应端点）；空清单由 `todo_write` 空列表事件自然收敛（面板随之不占位）。
- 页面加载仍有一条 404 静态资源请求（与本方案无关，待单独排查）。
