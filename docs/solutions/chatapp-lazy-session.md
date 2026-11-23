---
title: chatapp 新建会话惰性创建方案（对齐 dsh）
description: 点击「新建会话」只进入草稿态（不落盘、不进列表、不占历史），首条消息发送时才真正创建会话；同时隐藏历史遗留空会话，并给出侧边栏草稿态展开规则与 busy 时序修复。
keywords: chatapp, 新建会话, 惰性创建, draft, eventCount, 会话列表, dsh
author: AI 助手
date: 2026-11-11
updated: 2026-11-11
status: done
category: solutions
---

# chatapp 新建会话惰性创建方案（对齐 dsh）

## 1. 问题

chatapp 的「新建会话」此前是 **eager**（即时）创建：点一下就直接 `POST /v1/sessions`，REST 立即落盘并返回会话 id。由此产生三类问题：

1. **空会话谷底**：用户点「新建会话」后只是看了看、没发消息，磁盘上就多出一条 `新会话 HHMMSS` 的空会话；实测 `.onion-test` 里已堆积 18 条 `eventCount === 0` 的会话。
2. **列表污染**：空会话与真实会话并列显示在侧边栏，难以区分「我还没说话的空白页」和「历史记录」。
3. **启动落点错误**：挂载时 `attach(list[0].id)` 直接恢复最近会话，而最近一条往往正是刚点出来的空会话，打开应用即落在空白页上。

dsh 的做法是 **lazy draft（惰性草稿）**：点「新建会话」只打开一个空白视图，会话在首条消息发送时才真正创建（也就没有「空会话」这个概念）。

## 2. 方案

### 2.1 草稿态（draft）

`chat-view.tsx` 的 `newSession(cwd?)` 不再调 `agent.createSession`，只把视图切到空白草稿：

- 解绑当前会话订阅（`subRef.dispose()`）、`sessionRef.current = null`、`chatStore.setCurrent(null)`；
- 清空消息流 / 待回填 / goal / plan / todos / info / lastTurn / error；
- 记下草稿意图：`draftCwd`（工作区）+ 暂存的 `mode`（preset）；
- 工作区选择立即写入「近用工作区」（`recentWorkspaces`）。

草稿态下 `sessionId` 为空：`ApprovalChip` 保持禁用（没有会话可设预设），`ModelChip` 不依赖会话——展示 `info?.model ?? draftModel`，改动只记偏好（`setDraftModel` + 持久化 `lastModel`），等会话创建时套用。

### 2.2 首条消息才创建（ensure）

`send()` 里 `ensure()` 承担创建职责（草稿 → 会话）：

```
createSession(draftCwd, mode === "creator" ? "creator" : undefined)
  → attach(id)             // 订阅 + 拉 session 详情
  → 若 draftModel 与默认模型不同，setSessionModel
  → 清 mode / draftCwd
```

`agent.send()` 成功后才 `refreshSessions()`——**会话列表在此刻才多出一行**（对齐 dsh：历史里只出现已经说过话的会话）。

### 2.3 会话列表过滤（显示逻辑）

`chatStore.setSessions` 在排序前过滤：

```ts
const visible = sessions.filter((s) => s.eventCount > 0 || s.id === state.currentId);
```

- `eventCount > 0` 即「至少落盘过一条事件」，为空则不显示（同时清掉历史遗留空会话）；
- 当前挂载会话**始终保留**，避免首条消息落盘前的短暂窗口里选中项凭空消失；
- 依赖该过滤的两处：chatapp 挂载时的 `attach(list[0].id)`（启动不再落到空会话）与侧边栏刷新。

### 2.4 侧边栏草稿态的展开规则

原侧边栏只在「包含当前会话的工作区组」应用「默认展开、点组头折叠」，其余组走 `expanded` 集合（默认收起）。草稿态 `currentId` 为空 → 没有任何「当前组」→ 点「新建会话」会让整个历史列表收起。

改为引入展开基准 `anchorKey`：

```ts
const currentKey = sessions.find((s) => s.id === currentId)?.workingDir;
const lastKeyRef = useRef(currentKey);
if (currentKey !== undefined) lastKeyRef.current = currentKey;
const anchorKey = currentKey ?? lastKeyRef.current;   // 草稿态沿用最后所在工作区
```

`open = group.key === anchorKey ? !collapsed.has(key) : expanded.has(key)`，组头点击同源。附带效果：连续点「新建会话」时草稿工作区不再丢失（`currentWorkspace = anchorKey`）。注意 `is-current` 样式仍只绑定真实当前会话，草稿态不高亮任何组。

### 2.5 busy 时序（草稿创建的副作用修复）

`attach()` 会重置视图状态（含 `busy = false`）。草稿路径下 `send()` 先置 busy、后 `ensure()`，于是 `attach` 把 busy 冲掉——UI 认为空闲（发送键回到禁用箭头态），而服务端轮次仍在跑。

修复：`busyRef.current = true` 提前置位**防重入**（草稿创建会话期间挡住重复发送），`setBusy(true)` 挪到 `ensure()` 之后（`attach` 重置完成后再置 UI 态）。

## 3. 验收

`npm run check` + `npm run build` 通过，`POST /v1/apps/install` 安装后 playwright 冒烟（`ava2-verify/lazy-session-smoke.mjs`，真实模型轮次）6 项全 PASS：

| # | 断言 | 实测 |
| --- | --- | --- |
| 0 | 启动恢复落在有内容的会话 | `onion-1794381097601` `eventCount=235` |
| 1 | 点「新建会话」不落盘 | 服务端会话数 74 → 74，`POST /v1/sessions` 0 次 |
| 2 | 草稿视图 | 空态「开始新的对话」，侧边栏行 5 → 5 未变 |
| 3 | 首条消息落盘 | 74 → 75，POST 1 次 |
| 4 | 空会话不显示 | 服务端 18 条 `eventCount=0` 会话均未出现在侧边栏 |
| 5 | 新会话进列表 | 最新会话 `onion-1794381275319`（`eventCount=15`）已入侧边栏 |

附注：草稿 chip 分别显示工作区 `C:/…/SuperAI4` 与默认模型 `Qwen3.8-27B`。

回归：发送键状态冒烟（`stop-button-smoke.mjs`）3/3 PASS（空闲=发送禁用、生成中=停止可点、停止后回到发送态，轮次头「已取消」）。

截图：`.screenshots/ava2-host/08-draft-empty.png`（草稿）、`08-first-send.png`（首条消息）、`08-after-first-message.png`（新会话入列表）、`07-stop-button.png`。

## 4. 已知边界

- 历史遗留的空会话只**隐藏**、未删除；如需清理须另行授权（服务端 `GET /v1/sessions` 仍会返回它们）。
- 草稿态切换审批预设不可用（无会话可写），审批 chip 保持禁用；模型偏好改为记在 `lastModel`，创建会话时套用。
- 生成过程中点「新建会话」会解绑订阅、丢弃该轮的在途渲染；轮次本身仍在服务端继续，重进该会话可从 transcript 回放。
- 会话自动命名由服务端在本轮结束后生成，命名完成前列表标签回落为 `新会话 <id 尾 6 位>`（这是有内容会话的临时标题，与空会话过滤无关）。
