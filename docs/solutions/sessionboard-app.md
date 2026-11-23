---
title: 会话工作台 App（3.1 第二个真实 App）
description: 用 canvas 三件套 + agent 通道 + storage/settings 全量契约实现第二个真实 App「会话工作台」，吃自己的狗粮验证 app-dev-guide 契约完备性。
keywords: sessionboard, 会话工作台, canvas 三件套, AgentChannel, 狗粮验证, AppHost
author: AI 助手
date: 2026-11-09
updated: 2026-11-09
status: done
category: solutions
---

# 会话工作台 App（3.1 第二个真实 App）

## 1. 背景与目标

roadmap 3.1：在 hello（契约冒烟）与 chatapp（完整对话）之后，再做一个真实 App，**吃自己的狗粮**验证 `app-dev-guide.md` 的契约完备性——凡 App 开发者会踩的坑（slot 形态、inject 面、事件帧、canvas 编排），本 App 都要真实走一遍，缺口反哺文档与 host API 版本。

选型对比：

| 候选 | 判定 |
| --- | --- |
| 会话总览增强版（本方案） | agent 通道是真实后端能力，对话往返可端到端验证；hello 的画布「会话总览」只是只读列表，增强空间真实存在 |
| 任务面板 | onion-server 无任务后端，只能做假数据，违背「真实 App」与真实 API 冒烟约定 |

## 2. 狗粮矩阵（契约消费面）

目标：一次覆盖全部五类 slot 与全部五类 inject 面。

| 契约面 | 消费方式 |
| --- | --- |
| `surface.main` | viewId `board`：会话仪表盘（统计 + 列表 + 新建/删除/刷新） |
| `surface.canvas` | viewId `live`：对话试验台（选会话 → 发消息 → 收流式回复），画布启动器卡片进入 |
| `nav.sidebar` | 入口（order 6，位于 hello 的 10 之前） |
| `panel.sidebar` | viewId `board`：会话速览（计数 + 最近会话） |
| `panel.canvas` | viewId `live`：运行状态（默认模型 + 会话忙闲 + 最近 turn_report） |
| `inject: slots` | 上述全部 |
| `inject: agent` | listSessions / createSession / deleteSession / send / onSessionEvent / getSession / listModels |
| `inject: events` | emit `canvas.open {viewId:"live"}`（主区一键开画布）+ App 内 `sessionboard.select {id}`（面板跟随选中）；on `ping` 观察总线 |
| `inject: storage` | 记忆上次选中会话 `lastSessionId`，重进恢复 |
| `inject: settings` | schema：`pageSize`（number，默认 8，1–50）列表条数上限；`confirmDelete`（boolean，默认 true）删除前确认 |

manifest：hostApi 2、`kind` 缺省（preset，画布启动器排在 demo 之前）、裸 ESM 形态（无构建，import map 供 react）。

## 3. 视图与交互设计

### 3.1 主区「会话工作台」（surface.main: board）

- 顶部统计行：总会话 / live / busy 三枚计数 + 「刷新」「新建会话」按钮。
- 会话列表（上限 `pageSize`）：标题、id、eventCount、创建时间；行操作「试验」→ emit `canvas.open` 打开画布试验台并预选该会话，「删除」→ deleteSession（404 幂等）后刷新。
- 新建会话：createSession() 后刷新并选中。

### 3.2 画布「对话试验台」（surface.canvas: live）

- 会话选择器：下拉列出会话 + 「新建」；选择变化时订阅 onSessionEvent，**首个 `snapshot` 帧构建历史**（与 chatapp 同构，不用 getMessages）。
- 对话区：user/assistant 气泡（简化渲染：只取 text 块，不渲染工具行——工具往返由 chatapp 承担）。
- 输入区：textarea + 发送 → `ctx.agent.send(id, text)`；发送中禁用。
- 状态机：`turn_start` → busy；`turn_end` / `turn_report` → idle；`assistant_attempt` → 显示重试提示。

### 3.3 事件帧消费规则（对齐 chatapp frame-reducer 已验证形状）

- `snapshot`：`frame.events[]` 逐条走 durable 管线。
- `durable`：`user_message`（跳过 `message.source` 非空的宿主注入帧）→ 用户气泡；`assistant_message` → `message.blocks[].type==="text"` 拼接为回答（整段替换，非追加）；`turn_end` → 收束。
- `live`：`event.AssistantStream.Text` 增量追加（跳过 `Durable` 包装帧与 `End` 字符串变体）。
- 其余帧（approval/question/tool_*）本 App 不消费，静默忽略——试验台只验证文本往返。

### 3.4 panel.canvas「运行状态」

默认模型（listModels）+ 当前选中会话忙闲 + 最近一次 turn_report（turn/steps/reason）。

## 4. 契约观察与缺口（狗粮结论，已反哺 app-dev-guide）

冒烟（CDP 驱动 headless Edge，全真实链路）暴露的契约观察：

1. **`listSessions` 排序未定义**（缺口）：返回顺序由 onion 决定（实测旧会话在前），主区 `pageSize` 截断后**新建会话可能不可见**；`SessionListItem.createdAtMs` 为可选字段，App 无法可靠自行排序。→ guide §6 已标注「顺序未定义」。
2. **空闲会话无 busy 信号**（缺口）：订阅事件流后若会话空闲，snapshot 之前没有任何状态帧，面板只能显示「连接中」；首个 `snapshot` 帧到达 = 连接就绪且无进行中轮次。→ guide §6 已标注「snapshot 既是历史也是就绪信号」。本 App 的 panel.canvas 已按此修复（snapshot → 空闲，live AssistantStream → 回复中）。
3. **画布 tab 切换卸载视图状态**（观察）：canvas 折叠/展开不卸载组件（状态驻留），但 **tab 间切换会卸载**非激活视图——试验台 transcript 丢失，重进靠 snapshot 重建历史。→ guide §4 已标注。
4. **面板跟随模式**（模式沉淀）：`panel.*` 与对应 `surface.*` 是独立组件实例，无法共享 state；跨视图协同走 `ctx.storage`（持久）+ 自定义总线事件（瞬时，如本 App 的 `sessionboard.select {id}`）。→ guide §7 已沉淀为模式。
5. **版本指纹流程验证有效**：改产物 → bump `version` → `POST /v1/apps/install` overwrite → 新模块生效（guide §9 已有文档，本次实测顺畅，v0.1.0→0.1.1）。

## 5. 验收清单（2026-11-09 冒烟通过 ✅）

1. ✅ REST 安装 `POST /v1/apps/install`（overwrite）成功，App 管理面板显示「运行中」。
2. ✅ 侧栏出现「会话工作台」入口，主区仪表盘列出真实会话（统计：总数 67 / live 4 / 空闲 63）。
3. ✅ 新建会话 → REST 真实创建（提示「已新建会话 onion-…」）；列表截断观察见 §4.1。
4. ✅ 画布启动器「对话试验台」卡片打开；panel.canvas「运行状态」渲染（默认模型 Qwen3.8-27B + 空闲态）。
5. ✅ 试验台发送「你好，请用一句话自我介绍」→ agent 通道真实往返 → 助手回复渲染（Qwen3.8-27B）。
6. ✅ storage：「试验」打开试验台自动选中 lastSessionId（select 值与主区行 id 一致）。
7. ✅ 回归：对话 / 冒烟 / App 管理 导航齐备，chatapp / hello / multipage-demo 不受影响。

settings 链路（schema 表单渲染/保存→重载→`ctx.settings` 注出）由 2.2 在 multipage-demo 已冒烟，本 App 消费方式同构（`ctx.settings.get("pageSize") / get("confirmDelete")`），未重复端到端验证。

冒烟脚本：`.ava2-smoke/sessionboard-smoke.mjs`（不入库）；截图 `01-shell` ~ `04-reply` 存 `.ava2-smoke/shots/`。
